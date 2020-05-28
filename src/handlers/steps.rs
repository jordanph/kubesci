use crate::handlers::{ErrorMessage, Step, StepStatus};
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::{
    api::{Api, ListParams},
    Client,
};
use log::error;
use serde_json::json;
use std::convert::Infallible;
use warp::http::StatusCode;

pub async fn handle_get_steps(
    pipeline_name: String,
    commit: String,
) -> Result<impl warp::Reply, Infallible> {
    match get_pipeline_steps(pipeline_name, commit).await {
        Ok(pods) => {
            let json = warp::reply::json(&pods);

            Ok(warp::reply::with_status(json, StatusCode::OK))
        }
        Err(error) => {
            error!("Unexpected error occurred: {}", error);

            let json = warp::reply::json(&ErrorMessage { code: 500 });

            Ok(warp::reply::with_status(
                json,
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

async fn get_pipeline_steps(
    pipeline_name: String,
    commit: String,
) -> Result<serde_json::value::Value, Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into());

    let pods_api: Api<Pod> = Api::namespaced(client, &namespace);

    let labels = format!("app=kubes-cd-test,repo={},commit={}", pipeline_name, commit);

    let list_params = ListParams::default().labels(&labels);

    let pods_response = pods_api.list(&list_params).await?;

    let maybe_pod: Option<&Pod> = pods_response.items.first();

    let steps = maybe_pod.map(|pod| extract_steps(pod));

    Ok(json!(steps))
}

fn extract_steps(pod: &Pod) -> Vec<Step> {
    pod.status
        .clone()
        .unwrap()
        .container_statuses
        .unwrap()
        .iter()
        .map(|container| {
            let name = container.name.clone();
            let state = container.state.clone().unwrap();

            if let Some(running) = state.running {
                let Time(started_at) = running.started_at.unwrap();

                Step {
                    name,
                    status: Some(StepStatus {
                        started_at: Some(started_at),
                        finished_at: None,
                        status: "Running".to_string(),
                    }),
                }
            } else if state.waiting.is_some() {
                Step {
                    name,
                    status: Some(StepStatus {
                        started_at: None,
                        finished_at: None,
                        status: "Pending".to_string(),
                    }),
                }
            } else if let Some(terminated) = state.terminated {
                let Time(started_at) = terminated.started_at.unwrap();
                let Time(finished_at) = terminated.finished_at.unwrap();

                let status = if terminated.exit_code == 0 {
                    "Succeeded".to_string()
                } else {
                    "Failed".to_string()
                };

                Step {
                    name,
                    status: Some(StepStatus {
                        started_at: Some(started_at),
                        finished_at: Some(finished_at),
                        status,
                    }),
                }
            } else {
                Step { name, status: None }
            }
        })
        .collect()
}
