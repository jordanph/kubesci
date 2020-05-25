use crate::github::auth::authenticate_app;
use crate::github::client::auth::GithubAuthorisationClient;
use crate::github::client::installation::GithubInstallationClient;
use crate::pipeline::generate::generate_kubernetes_pipeline;
use crate::pipeline::steps_filter::filter;
use crate::pipeline::RawPipeline;
use crate::pipeline::StepWithCheckRunId;
use crate::routes::PodSuccessfullyFinishedRequest;
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

pub async fn handle_pod_finished_successfully_request(
    installation_id: u32,
    pod_finished_successfully_request: PodSuccessfullyFinishedRequest,
) -> Result<impl warp::Reply, Infallible> {
    match handle_pod_finished_successfully(installation_id, pod_finished_successfully_request).await
    {
        Ok(()) => Ok(warp::reply::with_status("".to_string(), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            error.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn handle_pod_finished_successfully(
    installation_id: u32,
    pod_finished_successfully_request: PodSuccessfullyFinishedRequest,
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
        .get_installation_access_token(installation_id)
        .await?;

    let github_installation_client = GithubInstallationClient {
        repository_name: &pod_finished_successfully_request.repo_name,
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    let maybe_raw_pipeline = github_installation_client
        .get_pipeline_file(&pod_finished_successfully_request.commit_sha)
        .await?;

    if let Some(raw_pipeline) = maybe_raw_pipeline {
        let raw_pipeline: RawPipeline = serde_yaml::from_str(&raw_pipeline)?;

        let previous_step_section: usize = pod_finished_successfully_request.step_section;
        let next_step_section = previous_step_section + 1;

        let maybe_steps = filter(
            &raw_pipeline.steps,
            &pod_finished_successfully_request.branch_name,
            next_step_section,
        );

        if let Some(Right(steps)) = maybe_steps {
            let mut steps_with_check_run_id: Vec<StepWithCheckRunId> = Vec::new();

            for step in steps {
                let checkrun_response = github_installation_client
                    .create_check_run(&step.name, &pod_finished_successfully_request.commit_sha)
                    .await?;

                steps_with_check_run_id.push(StepWithCheckRunId {
                    step,
                    check_run_id: checkrun_response.id,
                });
            }

            let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into());

            let pod_deployment = generate_kubernetes_pipeline(
                &steps_with_check_run_id,
                &pod_finished_successfully_request.commit_sha,
                &pod_finished_successfully_request.repo_name,
                &namespace,
                installation_id,
                next_step_section,
                &pod_finished_successfully_request.branch_name,
            );

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
        } else if let Some(Left(block)) = maybe_steps {
            github_installation_client
                .create_block_step(
                    &block.name,
                    &pod_finished_successfully_request.commit_sha,
                    next_step_section + 1,
                )
                .await?;
        }
    }
    Ok(())
}
