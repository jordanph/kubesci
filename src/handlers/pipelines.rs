use k8s_openapi::api::core::v1::Pod;
use itertools::Itertools;
use kube::{
  api::{Api, ListParams},
  Client,
};
use crate::handlers::{ErrorMessage, Pipeline, extract_runs};
use serde_json::json;
use std::convert::Infallible;
use warp::http::StatusCode;
use log::error;

pub async fn handle_get_pipelines() -> Result<impl warp::Reply, Infallible> {
  match get_pipelines().await {
      Ok(pods) => {
          let json = warp::reply::json(&pods);

          Ok(warp::reply::with_status(json, StatusCode::OK))
      },
      Err(error) => {
          error!("Unexpected error occurred: {}", error);

          let json = warp::reply::json(&ErrorMessage {
              code: 500
          });

          Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
      },
  }
}

async fn get_pipelines() -> Result<serde_json::value::Value, Box<dyn std::error::Error>> {
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