use crate::pipeline::KubernetesContainer;
use k8s_openapi::api::core::v1::{Container, EnvVar};

pub struct PollingSidecarContainer<'a> {
    pub installation_id: u32,
    pub namespace: &'a str,
    pub repo_name: &'a str,
    pub commit_sha: &'a str,
    pub step_section: usize,
    pub branch: &'a str,
}

impl<'a> KubernetesContainer for PollingSidecarContainer<'a> {
    fn to_container(&self) -> Container {
        let env = vec![
            EnvVar {
                name: "POD_NAME".to_string(),
                value: Some(format!(
                    "{}-{}",
                    self.commit_sha.to_string(),
                    self.step_section.to_string()
                )),
                value_from: None,
            },
            EnvVar {
                name: "COMMIT_SHA".to_string(),
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
            EnvVar {
                name: "STEP_SECTION".to_string(),
                value: Some(self.step_section.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "BRANCH_NAME".to_string(),
                value: Some(self.branch.to_string()),
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
