use crate::pipeline::KubernetesContainer;
use k8s_openapi::api::core::v1::{Container, EnvVar, VolumeMount};

pub struct GitInitContainer<'a> {
    pub clone_url: &'a str,
    pub commit_sha: &'a str,
    pub volume_mount_names: &'a Vec<String>,
}

impl<'a> KubernetesContainer for GitInitContainer<'a> {
    fn to_container(&self) -> Container {
        let container_volumes = self
            .volume_mount_names
            .iter()
            .map(|volume_name| format!("/{}", volume_name))
            .collect::<Vec<String>>()
            .join(";");

        let volume_mounts = self
            .volume_mount_names
            .iter()
            .map(|volume_name| VolumeMount {
                mount_path: format!("/{}", volume_name),
                mount_propagation: None,
                name: volume_name.to_string(),
                read_only: None,
                sub_path: None,
                sub_path_expr: None,
            })
            .collect::<Vec<VolumeMount>>();

        let env = vec![
            EnvVar {
                name: "REPO_URL".to_string(),
                value: Some(self.clone_url.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "COMMIT_SHA".to_string(),
                value: Some(self.commit_sha.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "CONTAINER_VOLUMES".to_string(),
                value: Some(container_volumes),
                value_from: None,
            },
        ];

        Container {
            args: None,
            command: None,
            env: Some(env),
            env_from: None,
            image: Some("jordanph/kubes-cd-git-checkout:latest".to_string()),
            image_pull_policy: None,
            lifecycle: None,
            liveness_probe: None,
            name: "kubes-cd-git-checkout".to_string(),
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
            working_dir: Some("/app".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn should_correctly_set_repo_url_as_env_variable() {
        let repo_name = "test-repo/test";

        let git_init_container = GitInitContainer {
            clone_url: &repo_name.to_string(),
            commit_sha: &"whatever".to_string(),
            volume_mount_names: &vec!["".to_string()],
        };

        let container = git_init_container.to_container();

        let envs = container.env.unwrap();

        let repo_url_env = envs.get(0).unwrap();

        let expected_value = EnvVar {
            name: "REPO_URL".to_string(),
            value: Some(repo_name.to_string()),
            value_from: None,
        };

        assert_eq!(repo_url_env, &expected_value)
    }

    #[test]
    fn should_correctly_serialize_commit_sha_as_env_variable() {
        let commit_sha = "f1fsaf13";

        let git_init_container = GitInitContainer {
            clone_url: &"whatever".to_string(),
            commit_sha: &commit_sha.to_string(),
            volume_mount_names: &vec!["".to_string()],
        };

        let container = git_init_container.to_container();

        let envs = container.env.unwrap();

        let commit_sha_env = envs.get(1).unwrap();

        let expected_value = EnvVar {
            name: "COMMIT_SHA".to_string(),
            value: Some(commit_sha.to_string()),
            value_from: None,
        };

        assert_eq!(commit_sha_env, &expected_value)
    }

    #[test]
    fn should_separate_container_volumes_env_by_semicolon() {
        let container_volume_names = vec![
            "test".to_string(),
            "google".to_string(),
            "house".to_string(),
        ];

        let git_init_container = GitInitContainer {
            clone_url: &"whatever".to_string(),
            commit_sha: &"commit_sha".to_string(),
            volume_mount_names: &container_volume_names,
        };

        let container = git_init_container.to_container();

        let envs = container.env.unwrap();

        let container_volumes_env = envs.get(2).unwrap();

        let expected_value = EnvVar {
            name: "CONTAINER_VOLUMES".to_string(),
            value: Some("/test;/google;/house".to_string()),
            value_from: None,
        };

        assert_eq!(container_volumes_env, &expected_value)
    }

    #[test]
    fn should_correct_construct_volume_mounts() {
        let container_volume_names = vec![
            "test".to_string(),
            "google".to_string(),
            "house".to_string(),
        ];

        let git_init_container = GitInitContainer {
            clone_url: &"whatever".to_string(),
            commit_sha: &"commit_sha".to_string(),
            volume_mount_names: &container_volume_names,
        };

        let container = git_init_container.to_container();

        let container_volumes_env = container.volume_mounts.unwrap();

        let expected_value = vec![
            VolumeMount {
                mount_path: "/test".to_string(),
                mount_propagation: None,
                name: "test".to_string(),
                read_only: None,
                sub_path: None,
                sub_path_expr: None,
            },
            VolumeMount {
                mount_path: "/google".to_string(),
                mount_propagation: None,
                name: "google".to_string(),
                read_only: None,
                sub_path: None,
                sub_path_expr: None,
            },
            VolumeMount {
                mount_path: "/house".to_string(),
                mount_propagation: None,
                name: "house".to_string(),
                read_only: None,
                sub_path: None,
                sub_path_expr: None,
            },
        ];

        assert_eq!(container_volumes_env, expected_value)
    }
}
