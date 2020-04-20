use k8s_openapi::api::core::v1::Pod;
use kube::{
  api::{Api, ListParams},
  Client,
};
use log::info;
use serde_json::json;


pub async fn get_pipelines() -> Result<serde_json::value::Value, Box<dyn std::error::Error>> {
  let client = Client::infer().await?;
  let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

  let pods_api: Api<Pod> = Api::namespaced(client, &namespace);

  let list_params = ListParams::default().labels("app=kubes-cd-test");

  let pods_response = pods_api.list(&list_params).await?;

  let pods: Vec<Pod> = pods_response.items;

  info!("Got pods {}", json!(pods));

  Ok(json!(pods))
}
