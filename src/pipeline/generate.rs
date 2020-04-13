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

pub fn generate_kubernetes_pipeline(pipeline: &Pipeline, github_head_sha: &String, repo_name: &String) -> Result<Pod, Box<dyn std::error::Error>> {
    let containers: serde_json::value::Value = pipeline.steps.iter().map(|step| {
        return json!({
            "name": step.name.replace(" ", "-").to_lowercase(),
            "image": step.image,
            "command": step.commands,
            "workingDir": "/app",
            "volumeMounts": [{
                "name": "repo",
                "mountPath": "/app",
            }]
        });
    }).collect();

    info!("Containers to deploy {}", containers);

    let clone_url = format!("https://github.com/{}", repo_name);

    let pod_deployment_config: Pod = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": github_head_sha },
        "spec": {
            "initContainers": [{
                "name": "cd-setup",
                "image": "alpine/git",
                "command": ["/bin/sh", "-c"],
                "args": [format!("git clone {} . && git checkout {}", clone_url, github_head_sha)],
                "workingDir": "/app",
                "volumeMounts": [{
                    "name": "repo",
                    "mountPath": "/app",
                }]
            }],
            "containers": containers,
            "restartPolicy": "Never",
            "volumes": [{
                "name": "repo",
                "emptyDir": {}
            }]
        }
    }))?;

    return Ok(pod_deployment_config);
}

pub fn generate_pipeline(raw_pipeline: &String) -> Result<Pipeline, Box<dyn std::error::Error>> {
    let yaml_steps: Pipeline = serde_yaml::from_str(&raw_pipeline)?;

    return Ok(yaml_steps);
}
