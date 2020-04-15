use serde_json::json;
use serde_derive::{Deserialize, Serialize};
use log::info;

use k8s_openapi::api::core::v1::Pod;

#[derive(Debug, Deserialize, Serialize)]
struct BasicEnv {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SecretKeyRef {
    name: String,
    key: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ValueFrom {
    #[serde(rename="secretKeyRef")]
    secret_key_ref: SecretKeyRef
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum Environment {
    BasicEnv { name: String, value: String},
    KubernetesSecretEnv {
        name: String,
        #[serde(rename="valueFrom")]
        value_from: ValueFrom
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Step {
    pub name: String,
    image: String,
    commands: std::vec::Vec<String>,
    branch: Option<String>,
    env: Option<std::vec::Vec<Environment>>,
}

#[derive(Debug, Deserialize)]
pub struct Pipeline {
    pub steps: std::vec::Vec<Step>,
}

pub fn generate_kubernetes_pipeline(pipeline: &Pipeline, github_head_sha: &String, repo_name: &String, github_branch_name: &String,) -> Result<Pod, Box<dyn std::error::Error>> {
    let containers: serde_json::value::Value =
        pipeline
            .steps
            .iter()
            .filter(|step| skip_step(step, github_branch_name))
            .map(|step| {
                let env = json!(step.env);

                return json!({
                    "name": step.name.replace(" ", "-").to_lowercase(),
                    "image": step.image,
                    "command": step.commands,
                    "workingDir": "/app",
                    "volumeMounts": [{
                        "name": "repo",
                        "mountPath": "/app",
                    }],
                    "env": env
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

fn skip_step(step: &Step, github_branch_name: &String) -> bool {
    return step.branch.is_none() || step.branch == Some(github_branch_name.to_string()) || not_branch(step.branch.as_ref(), github_branch_name);
}

fn not_branch(branch: Option<&String>, github_branch_name: &String) -> bool {
    return branch.map(|branch| branch.chars().next() == Some('!') && branch[1..] != github_branch_name.to_string()).unwrap_or(false);
}
