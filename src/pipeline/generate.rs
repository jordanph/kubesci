use crate::pipeline::init_containers::git::GitInitContainer;
use crate::pipeline::KubernetesContainer;
use crate::pipeline::StepWithCheckRunId;
use log::info;
use serde_json::json;
use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::{
    Container, EmptyDirVolumeSource, Pod, PodSpec, SecretVolumeSource, Volume,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

pub fn generate_kubernetes_pipeline(
    steps_with_check_run_id: &[StepWithCheckRunId],
    github_head_sha: &str,
    repo_name: &str,
    namespace: &str,
    _installation_id: u32,
    step_section: usize,
    _branch: &str,
) -> Pod {
    let containers: Vec<Container> = steps_with_check_run_id
        .iter()
        .map(|step_with_check_run_id| step_with_check_run_id.to_container())
        .collect();

    info!("Containers to deploy: {}", json!(containers));

    let volume_mount_names: Vec<String> = steps_with_check_run_id
        .iter()
        .map(|step_with_check_run_id| step_with_check_run_id.check_run_id.to_string())
        .collect();

    let mut secret_mounts: Vec<Volume> = steps_with_check_run_id
        .iter()
        .filter_map(|step_with_check_run_id| step_with_check_run_id.step.mount_secret.as_ref())
        .flatten()
        .map(|mount_secret| Volume {
            aws_elastic_block_store: None,
            azure_disk: None,
            azure_file: None,
            cephfs: None,
            cinder: None,
            config_map: None,
            csi: None,
            downward_api: None,
            empty_dir: None,
            fc: None,
            flex_volume: None,
            flocker: None,
            gce_persistent_disk: None,
            git_repo: None,
            glusterfs: None,
            host_path: None,
            iscsi: None,
            name: mount_secret.name.to_string(),
            nfs: None,
            persistent_volume_claim: None,
            photon_persistent_disk: None,
            portworx_volume: None,
            projected: None,
            quobyte: None,
            rbd: None,
            scale_io: None,
            secret: Some(SecretVolumeSource {
                default_mode: None,
                items: None,
                optional: Some(true),
                secret_name: Some(mount_secret.name.to_string()),
            }),
            storageos: None,
            vsphere_volume: None,
        })
        .collect();

    // Ensure the secret mounts are deduped if used more than once in pipeline
    secret_mounts.sort_by(|m1, m2| m1.name.partial_cmp(&m2.name).unwrap());
    secret_mounts.dedup_by(|m1, m2| m1.name.eq(&m2.name));

    let container_repo_volume_mounts: Vec<Volume> = volume_mount_names
        .clone()
        .iter()
        .map(|check_run_id| Volume {
            aws_elastic_block_store: None,
            azure_disk: None,
            azure_file: None,
            cephfs: None,
            cinder: None,
            config_map: None,
            csi: None,
            downward_api: None,
            empty_dir: Some(EmptyDirVolumeSource {
                medium: None,
                size_limit: None,
            }),
            fc: None,
            flex_volume: None,
            flocker: None,
            gce_persistent_disk: None,
            git_repo: None,
            glusterfs: None,
            host_path: None,
            iscsi: None,
            name: check_run_id.clone(),
            nfs: None,
            persistent_volume_claim: None,
            photon_persistent_disk: None,
            portworx_volume: None,
            projected: None,
            quobyte: None,
            rbd: None,
            scale_io: None,
            secret: None,
            storageos: None,
            vsphere_volume: None,
        })
        .collect();

    let volumes = [secret_mounts, container_repo_volume_mounts].concat();

    info!("Volumes to deploy: {}", json!(volumes));

    let short_commit = &github_head_sha[0..7];
    let clone_url = format!("https://github.com/{}", repo_name);

    let git_checkout_init_container = GitInitContainer {
        clone_url: &clone_url,
        commit_sha: github_head_sha,
        volume_mount_names: &volume_mount_names,
    };

    let mut pod_labels = BTreeMap::new();
    pod_labels.insert("repo".to_string(), repo_name.replace("/", "."));
    pod_labels.insert("commit".to_string(), short_commit.to_string());
    pod_labels.insert("app".to_string(), "kubes-cd-test".to_string());

    let pod_deployment_config = Pod {
        metadata: Some(ObjectMeta {
            annotations: None,
            cluster_name: None,
            creation_timestamp: None,
            deletion_grace_period_seconds: None,
            deletion_timestamp: None,
            finalizers: None,
            generate_name: None,
            generation: None,
            labels: Some(pod_labels),
            managed_fields: None,
            name: Some(format!("{}-{}", github_head_sha, step_section)),
            namespace: Some(namespace.to_string()),
            owner_references: None,
            resource_version: None,
            self_link: None,
            uid: None,
        }),
        spec: Some(PodSpec {
            active_deadline_seconds: None,
            affinity: None,
            automount_service_account_token: None,
            containers,
            dns_config: None,
            dns_policy: None,
            enable_service_links: None,
            ephemeral_containers: None,
            host_aliases: None,
            host_ipc: None,
            host_network: None,
            host_pid: None,
            hostname: None,
            image_pull_secrets: None,
            init_containers: Some(vec![git_checkout_init_container.to_container()]),
            node_name: None,
            node_selector: None,
            overhead: None,
            preemption_policy: None,
            priority: None,
            priority_class_name: None,
            readiness_gates: None,
            restart_policy: Some("Never".to_string()),
            runtime_class_name: None,
            scheduler_name: None,
            security_context: None,
            service_account: Some("kubes-cd".to_string()),
            service_account_name: Some("kubes-cd".to_string()),
            share_process_namespace: None,
            subdomain: None,
            termination_grace_period_seconds: None,
            topology_spread_constraints: None,
            tolerations: None,
            volumes: Some(volumes),
        }),
        status: None,
    };

    info!(
        "Pod configuration to deploy: {}",
        json!(pod_deployment_config)
    );

    pod_deployment_config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{MountSecret, Step};

    #[test]
    fn should_remove_duplicate_secret_mounts() {
        let github_head_sha = "abcdefgh";
        let repo_name = "test_repo";
        let namespace = "default";
        let installation_id = 1234;
        let branch = "some-branch";

        let step1 = Step {
            name: "some-step".to_string(),
            image: "some-image".to_string(),
            commands: None,
            args: None,
            branch: None,
            env: None,
            mount_secret: Some(vec1![
                MountSecret {
                    name: "some-secret".to_string(),
                    mount_path: "some-path".to_string()
                },
                MountSecret {
                    name: "duplicate-secret".to_string(),
                    mount_path: "some-path".to_string()
                }
            ]),
        };

        let step2 = Step {
            name: "some-step".to_string(),
            image: "some-image".to_string(),
            commands: None,
            args: None,
            branch: None,
            env: None,
            mount_secret: Some(vec1![
                MountSecret {
                    name: "some-other-secret".to_string(),
                    mount_path: "some-path".to_string()
                },
                MountSecret {
                    name: "duplicate-secret".to_string(),
                    mount_path: "some-path".to_string()
                }
            ]),
        };

        let steps_with_check_run_id = vec![
            StepWithCheckRunId {
                step: &step1,
                check_run_id: 1234,
            },
            StepWithCheckRunId {
                step: &step2,
                check_run_id: 1234,
            },
        ];

        let result = generate_kubernetes_pipeline(
            &steps_with_check_run_id,
            github_head_sha,
            repo_name,
            namespace,
            installation_id,
            0,
            branch,
        );

        let secret_mounts = result.spec.unwrap().volumes.unwrap();

        let duplicate_secret_count = secret_mounts
            .iter()
            .filter(|mount| mount.name.eq("duplicate-secret"))
            .count();

        assert_eq!(duplicate_secret_count, 1);
    }
}
