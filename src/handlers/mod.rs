use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use serde_derive::Serialize;
use chrono::{DateTime, Utc};

use kube::{
  api::{Api, ListParams},
  Client,
};
use serde_json::json;
use itertools::Itertools;

#[derive(Serialize, Clone)]
struct Run {
  status: Option<String>,
  commit: String
}

#[derive(Serialize)]
struct Pipeline {
  name: String,
  runs: Vec<Run>,
}

#[derive(Serialize)]
struct StepStatus {
  started_at: Option<DateTime<Utc>>,
  finished_at: Option<DateTime<Utc>>,
  status: String
}

#[derive(Serialize)]
struct Step {
  name: String,
  status: Option<StepStatus>
}

pub async fn get_pipeline_steps(pipeline_name: String, commit: String) -> Result<serde_json::value::Value, Box<dyn std::error::Error>> {
  let client = Client::infer().await?;
  let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

  let pods_api: Api<Pod> = Api::namespaced(client, &namespace);

  let labels = format!("app=kubes-cd-test,repo={},commit={}", pipeline_name, commit);

  let list_params = ListParams::default().labels(&labels);

  let pods_response = pods_api.list(&list_params).await?;

  let maybe_pod: Option<&Pod> = pods_response.items.first();

  let steps = maybe_pod.map(|pod| extract_steps(pod));

  Ok(json!(steps))
}

pub async fn get_pipeline(pipeline_name: String) -> Result<serde_json::value::Value, Box<dyn std::error::Error>> {
  let client = Client::infer().await?;
  let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

  let pods_api: Api<Pod> = Api::namespaced(client, &namespace);

  let labels = format!("app=kubes-cd-test,repo={}", pipeline_name);

  let list_params = ListParams::default().labels(&labels);

  let pods_response = pods_api.list(&list_params).await?;

  let pods: Vec<Pod> = pods_response.items;

  let pods_grouped_by_repo = group_by_branch(&pods);

  Ok(json!(pods_grouped_by_repo))
}

pub async fn get_pipelines() -> Result<serde_json::value::Value, Box<dyn std::error::Error>> {
  let client = Client::infer().await?;
  let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

  let pods_api: Api<Pod> = Api::namespaced(client, &namespace);

  let list_params = ListParams::default().labels("app=kubes-cd-test");

  let pods_response = pods_api.list(&list_params).await?;

  let pods: Vec<Pod> = pods_response.items;

  let pods_grouped_by_repo = group_by_repo(&pods);

  Ok(json!(pods_grouped_by_repo))
}

fn group_by_repo(pods: &[Pod]) -> Vec<Pipeline> {
  pods
    .into_iter()
    .group_by(|&pod| pod.metadata.clone().unwrap().labels.unwrap().get("repo").unwrap().clone())
    .into_iter()
    .map(|(key, group)| {
      let last_ten_runs: Vec<&Pod> = group.collect();

      let runs = extract_runs(last_ten_runs);

      Pipeline {
        name: key,
        runs: runs
      }
    })
    .collect()
}

fn group_by_branch(pods: &[Pod]) -> Vec<Pipeline> {
  pods
    .into_iter()
    .group_by(|&pod| pod.metadata.clone().unwrap().labels.unwrap().get("branch").unwrap().clone())
    .into_iter()
    .map(|(key, group)| {
      let last_ten_runs: Vec<&Pod> = group.collect();

      let runs = extract_runs(last_ten_runs);

      Pipeline {
        name: key,
        runs: runs
      }
    })
    .collect()
}

fn extract_steps(pod: &Pod) -> Vec<Step> {
  pod.status.clone().unwrap().container_statuses.unwrap()
    .into_iter()
    .map(|container| {
      let name = container.name;
      let state = container.state.unwrap();

      if let Some(running) = state.running {
        let Time(started_at) = running.started_at.unwrap();

        return Step {
          name: name,
          status: Some(StepStatus {
            started_at: Some(started_at),
            finished_at: None,
            status: "Running".to_string()
          })
        }
      } else if let Some(_) = state.waiting {
        return Step {
          name: name,
          status: Some(StepStatus {
            started_at: None,
            finished_at: None,
            status: "Pending".to_string()
          })
        }
      } else if let Some(terminated) = state.terminated {
        let Time(started_at) = terminated.started_at.unwrap();
        let Time(finished_at) = terminated.finished_at.unwrap();

        let status = if terminated.exit_code == 0 {
          "Succeeded".to_string()
        } else {
          "Failed".to_string()
        };

        return Step {
          name: name,
          status: Some(StepStatus {
            started_at: Some(started_at),
            finished_at: Some(finished_at),
            status: status
          })
        }
      } else {
        return Step {
          name: name,
          status: None
        }
      }
    })
    .collect()
}

fn extract_runs(pod: Vec<&Pod>) -> Vec<Run> {
  pod
    .into_iter()
    .map(|pod| {
      let status = pod.status.clone().unwrap().phase;
      let commit = pod.metadata.clone().unwrap().labels.unwrap().get("commit").unwrap().clone();

      Run {
        status: status,
        commit: commit
      }
    })
    .collect()
}
