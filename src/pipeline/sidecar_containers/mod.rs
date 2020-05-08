use crate::pipeline::KubernetesContainer;
use crate::pipeline::StepWithCheckRunId;
use k8s_openapi::api::core::v1::{Container, EnvVar};

pub struct PollingSidecarContainer<'a> {
    pub installation_id: u32,
    pub namespace: &'a str,
    pub repo_name: &'a str,
    pub commit_sha: &'a str,
    pub steps_with_check_run_ids: &'a [StepWithCheckRunId<'a>],
}

impl<'a> KubernetesContainer for PollingSidecarContainer<'a> {
    fn to_container(&self) -> Container {
        let step_check_id_map_env: String = self
            .steps_with_check_run_ids
            .iter()
            .map(|step_with_check_run_id| {
                format!(
                    "{}={}",
                    step_with_check_run_id.step.name, step_with_check_run_id.check_run_id
                )
            })
            .collect::<Vec<String>>()
            .join(",");

        let env = vec![
            EnvVar {
                name: "CHECK_RUN_POD_NAME_MAP".to_string(),
                value: Some(step_check_id_map_env),
                value_from: None,
            },
            EnvVar {
                name: "POD_NAME".to_string(),
                value: Some(self.commit_sha.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "RUST_LOG".to_string(),
                value: Some("info".to_string()),
                value_from: None,
            },
            EnvVar {
                name: "INSTALLATION_ID".to_string(),
                value: Some(self.installation_id.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "KUBES_CD_CONTROLLER_BASE_URL".to_string(),
                value: Some("http://kubes-cd-controller".to_string()),
                value_from: None,
            },
            EnvVar {
                name: "NAMESPACE".to_string(),
                value: Some(self.namespace.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "REPO_NAME".to_string(),
                value: Some(self.repo_name.to_string()),
                value_from: None,
            },
        ];

        Container {
            args: None,
            command: None,
            env: Some(env),
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
            volume_mounts: None,
            working_dir: None,
        }
    }
}
