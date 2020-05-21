pub mod generate;
pub mod init_containers;
pub mod sidecar_containers;
pub mod steps_filter;

use serde_derive::{Deserialize, Serialize};
use vec1::Vec1;

use k8s_openapi::api::core::v1::{Container, EnvVar, EnvVarSource, SecretKeySelector, VolumeMount};

pub trait KubernetesContainer {
    fn to_container(&self) -> Container;
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SecretKeyRef {
    name: String,
    key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ValueFrom {
    #[serde(rename = "secretKeyRef")]
    secret_key_ref: SecretKeyRef,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
enum Environment {
    BasicEnv {
        name: String,
        value: String,
    },
    KubernetesSecretEnv {
        name: String,
        #[serde(rename = "valueFrom")]
        value_from: ValueFrom,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MountSecret {
    name: String,
    #[serde(rename = "mountPath")]
    mount_path: String,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub name: String,
    image: String,
    commands: Option<std::vec::Vec<String>>,
    args: Option<std::vec::Vec<String>>,
    branch: Option<String>,
    env: Option<Vec1<Environment>>,
    #[serde(rename = "mountSecret")]
    mount_secret: Option<Vec1<MountSecret>>,
}

pub struct StepWithCheckRunId<'a> {
    pub step: &'a Step,
    pub check_run_id: u32,
}

impl<'a> KubernetesContainer for StepWithCheckRunId<'a> {
    fn to_container(&self) -> Container {
        let working_dir = "/app".to_string();

        let repo_mount = vec![VolumeMount {
            mount_path: working_dir.clone(),
            mount_propagation: None,
            name: self.check_run_id.to_string(),
            read_only: None,
            sub_path: None,
            sub_path_expr: None,
        }];

        let maybe_mounted_secrets = self.step.mount_secret.clone().map(|mount_secrets| {
            mount_secrets
                .into_iter()
                .map(|mount_secret| VolumeMount {
                    mount_path: mount_secret.mount_path,
                    mount_propagation: None,
                    name: mount_secret.name,
                    read_only: Some(true),
                    sub_path: None,
                    sub_path_expr: None,
                })
                .collect::<Vec<VolumeMount>>()
        });

        let volume_mounts: Vec<VolumeMount> = match maybe_mounted_secrets {
            Some(mount_secrets) => [mount_secrets.to_vec(), repo_mount].concat(),
            None => repo_mount,
        };

        let maybe_envs = self.step.env.clone().map(|envs| {
            envs.iter()
                .map(|env| match env {
                    Environment::BasicEnv { name, value } => EnvVar {
                        name: name.clone(),
                        value: Some(value.clone()),
                        value_from: None,
                    },
                    Environment::KubernetesSecretEnv { name, value_from } => EnvVar {
                        name: name.clone(),
                        value: None,
                        value_from: Some(EnvVarSource {
                            config_map_key_ref: None,
                            field_ref: None,
                            resource_field_ref: None,
                            secret_key_ref: Some(SecretKeySelector {
                                name: Some(value_from.secret_key_ref.name.clone()),
                                key: value_from.secret_key_ref.key.clone(),
                                optional: None,
                            }),
                        }),
                    },
                })
                .collect::<Vec<EnvVar>>()
        });

        let command = self.step.commands.as_ref().map(|commands| {
            let start_script_file = "#!/bin/sh\\nset -euf\\n".to_string();

            let mut script = start_script_file;

            for command in commands {
                script += &format!("echo '{}'\\n", command);
                script += &format!("{}\\n", command);
            }

            let escaped_script = script.replace("'", "'\\\''");

            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!(
                    "echo -e '{}' > ./script.sh && chmod +x ./script.sh && ./script.sh",
                    escaped_script
                ),
            ]
        });

        Container {
            args: self.step.args.clone(),
            command,
            env: maybe_envs,
            env_from: None,
            image: Some(self.step.image.to_string()),
            image_pull_policy: None,
            lifecycle: None,
            liveness_probe: None,
            name: self.step.name.replace(" ", "-").to_lowercase(),
            ports: None,
            readiness_probe: None,
            resources: None,
            security_context: None,
            stdin: None,
            stdin_once: None,
            termination_message_path: None,
            termination_message_policy: None,
            tty: None,
            volume_devices: None,
            volume_mounts: Some(volume_mounts),
            working_dir: Some(working_dir),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RawPipeline {
    pub steps: Vec1<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Pipeline {
    pub steps: Vec1<Step>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correctly_construct_script_to_run() {
        let step = Step {
            name: "test-string".to_string(),
            image: "some-image".to_string(),
            commands: Some(vec!["cargo test".to_string(), "cargo run".to_string()]),
            args: None,
            branch: None,
            env: None,
            mount_secret: None,
        };

        let step_with_check_run_id = StepWithCheckRunId {
            step: &step,
            check_run_id: 1,
        };

        let container = step_with_check_run_id.to_container();

        assert_eq!(container.command, Some(vec!("/bin/sh".to_string(), "-c".to_string(), "echo -e '#!/bin/sh\\nset -euf\\necho '\\\''cargo test'\\''\\ncargo test\\necho '\\''cargo run'\\''\\ncargo run\\n' > ./script.sh && chmod +x ./script.sh && ./script.sh".to_string())))
    }
}
