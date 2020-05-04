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