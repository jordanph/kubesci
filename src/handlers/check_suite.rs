use crate::routes::GithubCheckSuiteRequest;
use warp::http::StatusCode;
use std::convert::Infallible;
use crate::github::client::auth::GithubAuthorisationClient;
use crate::github::client::installation::GithubInstallationClient;
use crate::github::auth::authenticate_app;
use crate::pipeline::generate::{generate_pipeline, filter_steps, generate_kubernetes_pipeline};
use log::info;
use k8s_openapi::api::core::v1::Pod;

use kube::{
  api::{Api, Meta, PostParams},
  Client,
};

pub async fn handle_check_suite_request(github_webhook_request: GithubCheckSuiteRequest) -> Result<impl warp::Reply, Infallible> {
  if github_webhook_request.action != "requested" {
      return Ok(warp::reply::with_status("".to_string(), StatusCode::OK));
  }

  match create_check_run(github_webhook_request).await {
      Ok(()) => Ok(warp::reply::with_status("".to_string(), StatusCode::OK)),
      Err(error) => Ok(warp::reply::with_status(error.to_string(), StatusCode::INTERNAL_SERVER_ERROR)),
  }
}


async fn create_check_run(github_webhook_request: GithubCheckSuiteRequest) -> Result<(), Box<dyn std::error::Error>> {
  let github_jwt_token = authenticate_app()?;

  let github_authorisation_client = GithubAuthorisationClient {
      github_jwt_token: github_jwt_token,
      base_url: "https://api.github.com".to_string(),
  };

  let installation_access_token = github_authorisation_client.get_installation_access_token(github_webhook_request.installation.id).await?;

  let github_installation_client = GithubInstallationClient {
      repository_name: github_webhook_request.repository.full_name.to_string(),
      github_installation_token: installation_access_token,
      base_url: "https://api.github.com".to_string(),
  };

  let maybe_raw_pipeline = github_installation_client.get_pipeline_file(&github_webhook_request.check_suite.head_sha).await?;

  if let Some(raw_pipeline) = maybe_raw_pipeline {
    let pipeline = generate_pipeline(&raw_pipeline)?;

    let maybe_steps = filter_steps(&pipeline.steps, &github_webhook_request.check_suite.head_branch);
  
    if let Some(steps) = maybe_steps {
        let mut check_run_ids: Vec<(String, i32)> = Vec::new();
  
        for step in &steps {
            let checkrun_response = github_installation_client.create_check_run(&step.name, &github_webhook_request.check_suite.head_sha).await?;
  
            check_run_ids.push((step.name.replace(" ", "-").to_lowercase(), checkrun_response.id));
        }
  
        let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());
  
        let pod_deployment = generate_kubernetes_pipeline(
            &steps,
            &github_webhook_request.check_suite.head_sha,
            &github_webhook_request.repository.full_name,
            &github_webhook_request.check_suite.head_branch,
            check_run_ids,
            &namespace,
            github_webhook_request.installation.id
        )?;
  
        let client = Client::infer().await?;
  
        let pods: Api<Pod> = Api::namespaced(client, &namespace);
  
        info!("Creating Pod for checks...");
  
        let pp = PostParams::default();
        match pods.create(&pp, &pod_deployment).await {
            Ok(o) => {
                let name = Meta::name(&o);
                info!("Created pod: {}!", name);
            }
            Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409),
            Err(e) => return Err(e.into()),
        }
    }
  }

  Ok(())
}
