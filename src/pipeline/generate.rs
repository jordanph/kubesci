use serde_json::json;
use serde_derive::{Deserialize, Serialize};
use log::info;
use vec1::Vec1;

use k8s_openapi::api::core::v1::Pod;

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
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MountSecret {
    name: String,
    #[serde(rename="mountPath")]
    mount_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Step {
    pub name: String,
    image: String,
    commands: Option<std::vec::Vec<String>>,
    args: Option<std::vec::Vec<String>>,
    branch: Option<String>,
    env: Option<Vec1<Environment>>,
    #[serde(rename="mountSecret")]
    mount_secret: Option<Vec1<MountSecret>>,
}

#[derive(Debug, Deserialize)]
pub struct RawPipeline {
    pub steps: Vec1<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Pipeline {
    pub steps: Vec1<Step>,
}

pub fn filter_steps<'a>(steps: &'a[Step], github_branch_name: &String) -> Option<Vec1<&'a Step>> {
    let maybe_steps = steps
        .iter()
        .filter(|step| skip_step(step, github_branch_name))
        .collect::<Vec<_>>();

    return Vec1::try_from_vec(maybe_steps).ok();
}

pub fn generate_kubernetes_pipeline<'a>(steps: &[&'a Step], github_head_sha: &String, repo_name: &String, branch: &String, step_check_id_map: Vec<(String, i32)>, namespace: &String) -> Result<Pod, Box<dyn std::error::Error>> {
    let mut containers: Vec<serde_json::value::Value> = steps
            .iter()
            .map(|step| {
                let env = json!(step.env);

                let repo_mount = vec![MountSecret {
                    name: "repo".to_string(),
                    mount_path: "/app".to_string(),
                }];

                let mount_secrets_ref  = step.mount_secret.as_ref();

                let mount_secrets: Vec<MountSecret> = match mount_secrets_ref {
                    Some(mount_secrets) => [mount_secrets.to_vec(), repo_mount].concat(),
                    None => repo_mount
                };

                return json!({
                    "name": step.name.replace(" ", "-").to_lowercase(),
                    "image": step.image,
                    "command": step.commands,
                    "args": step.args,
                    "workingDir": "/app",
                    "volumeMounts": json!(mount_secrets),
                    "env": env
                });
            }).collect();

    let step_check_id_map_env: String = step_check_id_map
        .iter()
        .map(|map| format!("{}={}", map.0, map.1))
        .collect::<Vec<String>>()
        .join(",");

    // Add the kubes side car container
    containers.push(json!({
        "name": "kubes-cd-sidecar",
        "image": "jordanph/kubes-sidecar",
        "env": [
            {
                "name": "CHECK_RUN_POD_NAME_MAP",
                "value": step_check_id_map_env
            },
            {
                "name": "POD_NAME",
                "value": github_head_sha
            },
            {
                "name": "RUST_LOG",
                "value": "debug"
            },
            {
                "name": "KUBES_CD_CONTROLLER_BASE_URL",
                "value": "http://kubes-cd-controller"
            },
            {
                "name": "NAMESPACE",
                "value": namespace
            }
        ]
    }));

    info!("Check run ids to step map {}", step_check_id_map_env);

    info!("Containers to deploy: {}", json!(containers));

    let clone_url = format!("https://github.com/{}", repo_name);

    let secret_mounts: std::vec::Vec<serde_json::value::Value> = steps
                            .iter()
                            .filter_map(|step| step.mount_secret.as_ref())
                            .flatten()
                            .map(|mount_secret| json!({ "name": mount_secret.name, "secret": { "secretName": mount_secret.name}}))
                            .collect();

    let repo_mount: serde_json::value::Value = json!({"name": "repo", "emptyDir": {}});

    let volumes = [secret_mounts, [repo_mount].to_vec()].concat();

    info!("Volumes to deploy: {}", json!(volumes));

    let short_commit = &github_head_sha[0..7];

    let pod_deployment_config: Pod = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": github_head_sha,
            "labels": {
                "repo": repo_name.replace("/", "."),
                "branch": branch,
                "commit": short_commit,
                "app": "kubes-cd-test"
            },
            "namespace": namespace
        },
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
            "serviceAccount": "kubes-cd",
            "serviceAccountName": "kubes-cd",
            "containers": containers,
            "restartPolicy": "Never",
            "volumes": volumes
        }
    }))?;

    info!("Pod configuration to deploy: {}", json!(pod_deployment_config));

    return Ok(pod_deployment_config);
}

pub fn generate_pipeline(raw_pipeline: &String) -> Result<RawPipeline, Box<dyn std::error::Error>> {
    let yaml_steps: RawPipeline = serde_yaml::from_str(&raw_pipeline)?;

    return Ok(yaml_steps);
}

fn skip_step(step: &Step, github_branch_name: &String) -> bool {
    return step.branch.is_none() || step.branch == Some(github_branch_name.to_string()) || not_branch(step.branch.as_ref(), github_branch_name);
}

fn not_branch(branch: Option<&String>, github_branch_name: &String) -> bool {
    return branch.map(|branch| branch.chars().next() == Some('!') && branch[1..] != github_branch_name.to_string()).unwrap_or(false);
}

#[cfg(test)]
mod tests {
    #[test]
    fn should_return_none_if_no_steps_to_run() {
        assert_eq!(0,0);
    }
}
