use crate::github::client::auth::GithubAuthorisationClient;
use crate::github::client::installation::GithubInstallationClient;
use crate::pipeline::PipelineService;
use crate::routes::CompleteCheckRunRequest;
use chrono::Utc;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Container;
use k8s_openapi::api::core::v1::ContainerStateTerminated;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::{
    api::{Api, DeleteParams, ListParams, LogParams, Meta, WatchEvent},
    runtime::Informer,
};
use log::{error, info};
use std::collections::HashMap;

#[derive(Clone)]
struct RunningPod {
    repo_name: String,
    installation_id: u32,
    commit_sha: String,
    branch_name: String,
    step_section: usize,
}

pub struct PodInformer {
    pub pipeline_service: PipelineService,
    pub pods_api: Api<Pod>,
    pub github_private_key: String,
    pub application_id: String,
    pub github_base_url: String,
}

impl PodInformer {
    pub async fn poll_pods(&self) {
        let inf = Informer::new(self.pods_api.clone()).params(ListParams::default().timeout(10));

        let mut running_pods: HashMap<String, RunningPod> = HashMap::new();

        loop {
            let mut pods = inf.poll().await.unwrap().boxed();

            while let Some(event) = pods.try_next().await.unwrap() {
                match self.handle_pod(event, &mut running_pods).await {
                    Ok(()) => {}
                    Err(e) => error!("Encountered error while polling pods: {}", e),
                }
            }
        }
    }

    async fn handle_pod(
        &self,
        ev: WatchEvent<Pod>,
        running_pods: &mut HashMap<String, RunningPod>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match ev {
            WatchEvent::Added(pod) => {
                info!("Pod was added: {}", pod.name());

                // TO DO: Add checks to clean up pipelines that finished when controller was down

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
                    running_pods.insert(pod.name(), running_pod.clone());
                }
            }
            WatchEvent::Modified(pod) => {
                info!("Pod was modified: {}", pod.name());

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
                                            if last_state.terminated.is_some() {
                                                None
                                            } else if let Some(terminated) =
                                                state.terminated.clone()
                                            {
                                                Some((container_status.name.clone(), terminated))
                                            } else {
                                                None
                                            }
                                        }
                                        (None, Some(state)) => {
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
                                let logs = self
                                    .get_container_logs(&pod.name(), &container.name)
                                    .await?;

                                let check_run_id = container
                                    .env
                                    .as_ref()
                                    .map(|env| env.iter().find(|env| env.name == "CHECK_RUN_ID"))
                                    .flatten()
                                    .unwrap()
                                    .value
                                    .as_ref()
                                    .unwrap();

                                let Time(finished_at) = finished_pod_state
                                    .finished_at
                                    .unwrap_or_else(|| Time(Utc::now()));

                                self.mark_step_complete(
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

                    if let Some(pod_phase) = pod
                        .status
                        .as_ref()
                        .map(|status| status.phase.as_ref())
                        .flatten()
                    {
                        if pod_phase == "Succeeded" || pod_phase == "Failed" {
                            self.pipeline_service
                                .start_step_section(
                                    running_pod.installation_id,
                                    &running_pod.repo_name,
                                    &running_pod.commit_sha,
                                    &running_pod.branch_name,
                                    running_pod.step_section,
                                )
                                .await?;

                            running_pods.remove(&pod.name());

                            self.delete_pod(&pod.name()).await?;
                        }
                    }
                }
            }
            WatchEvent::Deleted(pod) => {
                info!("Pod was deleted! {:?}", pod);
            }
            WatchEvent::Bookmark(_) => {}
            WatchEvent::Error(_e) => {}
        }
        Ok(())
    }

    async fn get_container_logs(
        &self,
        pod_name: &str,
        container_name: &str,
    ) -> Result<String, kube::error::Error> {
        let mut lp = LogParams::default();
        lp.follow = true;
        lp.timestamps = true;
        lp.container = Some(container_name.to_string());

        self.pods_api.logs(pod_name, &lp).await
    }

    async fn delete_pod(&self, pod_name: &str) -> Result<(), kube::error::Error> {
        let dp = DeleteParams::default();

        self.pods_api.delete(pod_name, &dp).await.map(|_result| ())
    }

    async fn mark_step_complete(
        &self,
        installation_id: u32,
        check_run_id: i32,
        repo_name: &str,
        logs: &str,
        finished_at: &str,
        successful: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let github_authorisation_client =
            GithubAuthorisationClient::new(&self.github_private_key, &self.application_id)?;

        let installation_access_token = github_authorisation_client
            .get_installation_access_token(installation_id)
            .await?;

        let github_installation_client = GithubInstallationClient {
            repository_name: repo_name,
            github_installation_token: installation_access_token,
            base_url: &self.github_base_url,
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
}
