pub mod steps_filter;

use crate::github::auth::authenticate_app;
use crate::github::client::auth::GithubAuthorisationClient;
use crate::github::client::installation::GithubInstallationClient;
use crate::kubernetes::generate::generate_pod_for_steps;
use crate::kubernetes::RawPipeline;
use crate::kubernetes::StepWithCheckRunId;
use crate::pipeline::steps_filter::filter;
use chrono::Utc;
use either::Either::{Left, Right};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, Meta, PostParams},
    Client,
};
use log::info;
use std::env;

pub async fn start_step_section(
    installation_id: u32,
    repo_name: &str,
    commit_sha: &str,
    branch_name: &str,
    step_section: usize,
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
        repository_name: repo_name,
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    let maybe_raw_pipeline = github_installation_client
        .get_pipeline_file(commit_sha)
        .await?;

    if let Some(raw_pipeline) = maybe_raw_pipeline {
        let raw_pipeline: RawPipeline = serde_yaml::from_str(&raw_pipeline)?;

        let previous_step_section: usize = step_section;
        let next_step_section = previous_step_section + 1;

        let maybe_steps = filter(&raw_pipeline.steps, branch_name, next_step_section);

        if let Some(Right(steps)) = maybe_steps {
            let mut steps_with_check_run_id: Vec<StepWithCheckRunId> =
                Vec::with_capacity(steps.len());

            for step in steps {
                let checkrun_response = github_installation_client
                    .create_check_run(&step.name, commit_sha)
                    .await?;

                steps_with_check_run_id.push(StepWithCheckRunId {
                    step,
                    check_run_id: checkrun_response.id,
                });
            }

            let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into());

            let pod_deployment = generate_pod_for_steps(
                &steps_with_check_run_id,
                commit_sha,
                repo_name,
                &namespace,
                installation_id,
                next_step_section,
                branch_name,
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
                .create_block_step(&block.name, commit_sha, next_step_section + 1)
                .await?;
        }
    }
    Ok(())
}
