pub mod generate;
pub mod init_containers;
pub mod sidecar_containers;
pub mod steps_filter;

use serde_derive::{Deserialize, Serialize};
use vec1::Vec1;

use k8s_openapi::api::core::v1::{Container, VolumeMount, EnvVar, EnvVarSource, SecretKeySelector};

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
    #[serde(rename="secretKeyRef")]
    secret_key_ref: SecretKeyRef
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize)]
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

pub struct StepWithCheckRunId<'a> {
    pub step: &'a Step,
    pub check_run_id: u32
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
            sub_path_expr: None
          }];

          let maybe_mounted_secrets = self.step.mount_secret.clone().map(|mount_secrets| mount_secrets.into_iter().map(|mount_secret| VolumeMount {
            mount_path: mount_secret.mount_path,
            mount_propagation: None,
            name: mount_secret.name,
            read_only: Some(true),
            sub_path: None,
            sub_path_expr: None
          }).collect::<Vec<VolumeMount>>());

        let volume_mounts: Vec<VolumeMount> = match maybe_mounted_secrets {
            Some(mount_secrets) => [mount_secrets.to_vec(), repo_mount].concat(),
            None => repo_mount
        };

        let maybe_envs = self.step.env.clone().map(|envs| envs.into_iter().map(|env| match env {
            Environment::BasicEnv {
                name, value
            } => EnvVar {
                name: name,
                value: Some(value),
                value_from: None
            },
            Environment::KubernetesSecretEnv {
                name, value_from
            } => EnvVar {
                name: name,
                value: None,
                value_from: Some(EnvVarSource {
                    config_map_key_ref: None,
                    field_ref: None,
                    resource_field_ref: None,
                    secret_key_ref: Some(SecretKeySelector {
                        name: Some(value_from.secret_key_ref.name),
                        key: value_from.secret_key_ref.key,
                        optional: None
                    })
                })
            }
        }).collect::<Vec<EnvVar>>());

        Container {
            args: self.step.args.clone(),
            command: self.step.commands.clone(),
            env: maybe_envs,
            env_from: None,
            image: Some("jordanph/kubes-sidecar:latest".to_string()),
            image_pull_policy: None,
            lifecycle: None,
            liveness_probe: None,
            name: "kubes-cd-sidecar".to_string(),
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
