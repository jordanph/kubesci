use crate::github::auth::authenticate_app;
use crate::github::client::auth::GithubAuthorisationClient;
use crate::github::client::installation::GithubInstallationClient;
use crate::pipeline::generate::generate_kubernetes_pipeline;
use crate::pipeline::steps_filter::filter;
use crate::pipeline::RawPipeline;
use crate::pipeline::StepWithCheckRunId;
use crate::routes::GithubCheckRunRequest;
use chrono::Utc;
use either::Either::{Left, Right};
use k8s_openapi::api::core::v1::Pod;
use log::info;
use std::convert::Infallible;
use std::env;
use warp::http::StatusCode;

use kube::{
    api::{Api, Meta, PostParams},
    Client,
};

pub async fn handle_check_run_request(
    github_webhook_request: GithubCheckRunRequest,
) -> Result<impl warp::Reply, Infallible> {
    if github_webhook_request.action != "requested_action" {
        return Ok(warp::reply::with_status("".to_string(), StatusCode::OK));
    }

    match handle_requested_action(github_webhook_request).await {
        Ok(()) => Ok(warp::reply::with_status("".to_string(), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            error.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn handle_requested_action(
    github_webhook_request: GithubCheckRunRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let github_private_key = env::var("GITHUB_APPLICATION_PRIVATE_KEY")?;
    let application_id = env::var("APPLICATION_ID")?;
    let now = Utc::now().timestamp();

    let github_jwt_token = authenticate_app(&github_private_key, &application_id, now)?;

    let github_authorisation_client = GithubAuthorisationClient {
        github_jwt_token,
        base_url: "https://api.github.com".to_string(),
    };

    let installation_access_token = github_authorisation_client
        .get_installation_access_token(github_webhook_request.installation.id)
        .await?;

    let github_installation_client = GithubInstallationClient {
        repository_name: &github_webhook_request.repository.full_name,
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    let maybe_raw_pipeline = github_installation_client
        .get_pipeline_file(&github_webhook_request.check_run.check_suite.head_sha)
        .await?;

    if let Some(raw_pipeline) = maybe_raw_pipeline {
        // TODO: Create a check run for parsing pipeline
        // Introduce "block" complexity
        let raw_pipeline: RawPipeline = serde_yaml::from_str(&raw_pipeline)?;

        let step_section: usize = github_webhook_request
            .requested_action
            .unwrap()
            .identifier
            .parse()
            .unwrap();

        let maybe_steps = filter(
            &raw_pipeline.steps,
            &github_webhook_request.check_run.check_suite.head_branch,
            step_section,
        );

        if let Some(Right(steps)) = maybe_steps {
            let mut steps_with_check_run_id: Vec<StepWithCheckRunId> = Vec::new();

            for step in steps {
                let checkrun_response = github_installation_client
                    .create_check_run(
                        &step.name,
                        &github_webhook_request.check_run.check_suite.head_sha,
                    )
                    .await?;

                steps_with_check_run_id.push(StepWithCheckRunId {
                    step,
                    check_run_id: checkrun_response.id,
                });
            }

            let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into());

            let pod_deployment = generate_kubernetes_pipeline(
                &steps_with_check_run_id,
                &github_webhook_request.check_run.check_suite.head_sha,
                &github_webhook_request.repository.full_name,
                &namespace,
                github_webhook_request.installation.id,
                step_section,
                &github_webhook_request.check_run.check_suite.head_branch,
            );

            let client = Client::try_default().await?;

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
        } else if let Some(Left(block)) = maybe_steps {
            github_installation_client
                .create_block_step(
                    &block.name,
                    &github_webhook_request.check_run.check_suite.head_sha,
                    step_section + 1,
                )
                .await?;
        }
    }
    Ok(())
}
