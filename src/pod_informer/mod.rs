use crate::github::auth::authenticate_app;
use crate::github::client::auth::GithubAuthorisationClient;
use crate::github::client::installation::GithubInstallationClient;
use crate::pipeline::generate::generate_kubernetes_pipeline;
use crate::pipeline::steps_filter::filter;
use crate::pipeline::RawPipeline;
use crate::pipeline::StepWithCheckRunId;
use crate::routes::CompleteCheckRunRequest;
use chrono::Utc;
use either::Either::{Left, Right};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Container;
use k8s_openapi::api::core::v1::ContainerStateTerminated;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::{
    api::{Api, DeleteParams, ListParams, LogParams, Meta, PostParams, WatchEvent},
    runtime::Informer,
    Client,
};
use log::info;
use std::collections::HashMap;
use std::env;

#[derive(Clone)]
struct RunningPod {
    repo_name: String,
    installation_id: u32,
    commit_sha: String,
    branch_name: String,
    step_section: usize,
}

pub async fn poll_pods() {
    let client = Client::try_default().await.unwrap();
    let pods: Api<Pod> = Api::namespaced(client, "kubesci");

    let inf = Informer::new(pods).params(ListParams::default().timeout(10));

    let mut running_pods: HashMap<String, RunningPod> = HashMap::new();

    loop {
        let mut pods = inf.poll().await.unwrap().boxed();

        while let Some(event) = pods.try_next().await.unwrap() {
            handle_pod(event, &mut running_pods).await.unwrap();
        }
    }
}

// This function lets the app handle an event from kube
async fn handle_pod(
    ev: WatchEvent<Pod>,
    running_pods: &mut HashMap<String, RunningPod>,
) -> Result<(), Box<dyn std::error::Error>> {
    match ev {
        WatchEvent::Added(pod) => {
            let maybe_running_pod = &pod
                .meta()
                .labels
                .as_ref()
                .map(|labels| {
                    let maybe_installation_id = labels.get("installation_id");
                    let maybe_repo_name = labels.get("repo_name");
                    let maybe_branch_name = labels.get("branch_name");
                    let maybe_commit_sha = labels.get("commit_sha");
                    let maybe_step_section = labels.get("step_section");

                    match (
                        maybe_installation_id,
                        maybe_repo_name,
                        maybe_branch_name,
                        maybe_commit_sha,
                        maybe_step_section,
                    ) {
                        (
                            Some(installation_id),
                            Some(repo_name),
                            Some(branch_name),
                            Some(commit_sha),
                            Some(step_section),
                        ) => Some(RunningPod {
                            installation_id: installation_id.clone().parse().unwrap(),
                            repo_name: repo_name.clone().replace(".", "/"),
                            branch_name: branch_name.clone(),
                            commit_sha: commit_sha.clone(),
                            step_section: step_section.clone().parse().unwrap(),
                        }),
                        _ => None,
                    }
                })
                .flatten();

            if let Some(running_pod) = maybe_running_pod {
                let pod_name = "some-name".to_string();

                running_pods.insert(pod_name, running_pod.clone());
            }
        }
        WatchEvent::Modified(pod) => {
            let maybe_pod = running_pods.get(&pod.name());

            if let Some(running_pod) = maybe_pod {
                // Check to see if one of the containers has finished comparing previous state
                let maybe_newly_finished_pods = pod
                    .status
                    .as_ref()
                    .map(|status| status.container_statuses.as_ref())
                    .flatten()
                    .map(|container_status| {
                        container_status
                            .iter()
                            .filter_map(|container_status| {
                                let maybe_last_state = container_status.last_state.as_ref();
                                let maybe_state = container_status.state.as_ref();

                                match (maybe_last_state, maybe_state) {
                                    (Some(last_state), Some(state)) => {
                                        if last_state == state {
                                            None
                                        } else if let Some(terminated) = state.terminated.clone() {
                                            Some((container_status.name.clone(), terminated))
                                        } else {
                                            None
                                        }
                                    }
                                    (_, Some(state)) => {
                                        if let Some(terminated) = state.terminated.clone() {
                                            Some((container_status.name.clone(), terminated))
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                }
                            })
                            .collect::<Vec<(String, ContainerStateTerminated)>>()
                    });

                if let Some(newly_finished_pods) = maybe_newly_finished_pods {
                    for (finished_pod_name, finished_pod_state) in newly_finished_pods {
                        if let Some(container) = pod
                            .spec
                            .as_ref()
                            .map(|pod_spec| pod_spec.containers.as_ref())
                            .map(|containers: &Vec<Container>| {
                                containers
                                    .iter()
                                    .find(|container| container.name == finished_pod_name)
                            })
                            .flatten()
                        {
                            let logs = get_container_logs(&pod.name(), &container.name).await?;

                            let check_run_id = container
                                .env
                                .as_ref()
                                .map(|env| env.iter().find(|env| env.name == "CHECK_RUN_ID"))
                                .flatten()
                                .unwrap()
                                .value
                                .as_ref()
                                .unwrap();

                            let Time(finished_at) =
                                finished_pod_state.finished_at.as_ref().unwrap();

                            mark_step_complete(
                                running_pod.installation_id,
                                check_run_id.parse().unwrap(),
                                &running_pod.repo_name,
                                &logs,
                                &finished_at.to_rfc3339(),
                                finished_pod_state.exit_code == 0,
                            )
                            .await?;
                        }
                    }
                }

                let all_containers_finished = pod
                    .status
                    .as_ref()
                    .map(|pod_status| pod_status.container_statuses.as_ref())
                    .flatten()
                    .map(|container_statuses| {
                        container_statuses.iter().all(|container_status| {
                            match container_status.state.as_ref() {
                                Some(state) => state.terminated.is_some(),
                                None => false,
                            }
                        })
                    })
                    .unwrap_or(false);

                if all_containers_finished {
                    kick_off_next_step(
                        running_pod.installation_id,
                        &running_pod.repo_name,
                        &running_pod.commit_sha,
                        &running_pod.branch_name,
                        running_pod.step_section,
                    )
                    .await?;

                    delete_pod(&pod.name()).await?;
                }
            }
        }
        WatchEvent::Deleted(pod) => {
            // Remove from the current running pods
            running_pods.remove(&pod.name());
            info!("Pod was deleted! {:?}", pod);
        }
        WatchEvent::Bookmark(_) => {}
        WatchEvent::Error(_e) => {}
    }
    Ok(())
}

async fn get_container_logs(
    pod_name: &str,
    container_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    let pods: Api<Pod> = Api::namespaced(client, "kubesci");

    let mut lp = LogParams::default();
    lp.follow = true;
    lp.timestamps = true;
    lp.container = Some(container_name.to_string());

    Ok(pods.logs(pod_name, &lp).await?)
}

async fn delete_pod(pod_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    let pods: Api<Pod> = Api::namespaced(client, "kubesci");

    let dp = DeleteParams::default();

    pods.delete(pod_name, &dp).await?;

    Ok(())
}

async fn mark_step_complete(
    installation_id: u32,
    check_run_id: i32,
    repo_name: &str,
    logs: &str,
    finished_at: &str,
    successful: bool,
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

    let check_run = github_installation_client
        .get_check_run(check_run_id)
        .await?;

    let conclusion = if successful { "success" } else { "failure" };

    let complete_check_run_request = CompleteCheckRunRequest {
        repo_name: repo_name.to_string(),
        check_run_id,
        status: "completed".to_string(),
        finished_at: Some(finished_at.to_string()),
        logs: logs.to_string(),
        conclusion: Some(conclusion.to_string()),
    };

    github_installation_client
        .set_check_run_complete(
            check_run_id,
            &complete_check_run_request,
            &check_run.name,
            &check_run.started_at,
        )
        .await?;

    Ok(())
}

async fn kick_off_next_step(
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
            let mut steps_with_check_run_id: Vec<StepWithCheckRunId> = Vec::new();

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

            let pod_deployment = generate_kubernetes_pipeline(
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
