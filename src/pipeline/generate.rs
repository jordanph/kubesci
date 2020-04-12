use serde_json::json;
use serde_derive::Deserialize;
use log::info;

use k8s_openapi::api::core::v1::Pod;

#[derive(Debug, Deserialize)]
pub struct Step {
    pub name: String,
    image: String,
    commands: std::vec::Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Pipeline {
    pub steps: std::vec::Vec<Step>,
}

pub fn generate_kubernetes_pipeline(pipeline: &Pipeline, github_head_sha: &String) -> Result<Pod, Box<dyn std::error::Error>> {
    let containers: serde_json::value::Value = pipeline.steps.iter().map(|step| {
        return json!({
            "name": step.name.replace(" ", "-").to_lowercase(),
            "image": step.image,
            "command": step.commands
        });
    }).collect();

    info!("Containers to deploy {}", containers);

    let pod_deployment_config: Pod = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": github_head_sha },
        "spec": {
            "containers": containers,
            "restartPolicy": "Never",
        }
    }))?;

    return Ok(pod_deployment_config);
}

pub fn generate_pipeline(raw_pipeline: &String) -> Result<Pipeline, Box<dyn std::error::Error>> {
    let yaml_steps: Pipeline = serde_yaml::from_str(&raw_pipeline)?;

    return Ok(yaml_steps);
}
