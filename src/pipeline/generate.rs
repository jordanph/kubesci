use serde_json::json;
use log::info;
use crate::pipeline::init_containers::git::GitInitContainer;
use crate::pipeline::sidecar_containers::PollingSidecarContainer;
use crate::pipeline::KubernetesContainer;
use crate::pipeline::StepWithCheckRunId;
use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::{Pod, Container, PodSpec, Volume, EmptyDirVolumeSource, SecretVolumeSource};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

pub fn generate_kubernetes_pipeline(steps_with_check_run_id: &[StepWithCheckRunId], github_head_sha: &str, repo_name: &str, branch: &str, namespace: &str, installation_id: u32) -> Pod {
    let mut containers: Vec<Container> = steps_with_check_run_id.iter().map(|step_with_check_run_id| step_with_check_run_id.to_container()).collect();

    let side_car_container = PollingSidecarContainer {
        installation_id,
        namespace,
        repo_name,
        commit_sha: github_head_sha,
        steps_with_check_run_ids: &steps_with_check_run_id
    };

    containers.push(side_car_container.to_container());
 
    info!("Containers to deploy: {}", json!(containers));

    let volume_mount_names: Vec<String> = steps_with_check_run_id
        .iter()
        .map(|step_with_check_run_id| step_with_check_run_id.check_run_id.to_string())
        .collect();

    let secret_mounts: Vec<Volume> = steps_with_check_run_id
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
                                    secret_name: Some(mount_secret.name.to_string())
                                }),
                                storageos: None,
                                vsphere_volume: None,
                            })
                            .collect();

    let container_repo_volume_mounts: Vec<Volume> = volume_mount_names.clone()
        .iter().map(|check_run_id| Volume {
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
                size_limit: None
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

    let git_checkout_init_container = GitInitContainer {
        clone_url: repo_name,
        commit_sha: github_head_sha,
        volume_mount_names: &volume_mount_names
    };

    let mut pod_labels = BTreeMap::new();
    pod_labels.insert("repo".to_string(), repo_name.replace("/", "."));
    pod_labels.insert("branch".to_string(), branch.to_string());
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
            initializers: None,
            labels: Some(pod_labels),
            managed_fields: None,
            name: Some(github_head_sha.to_string()),
            namespace: Some(namespace.to_string()),
            owner_references: None,
            resource_version: None,
            self_link: None,
            uid: None
        }),
        spec: Some(PodSpec {
            active_deadline_seconds: None,
            affinity: None,
            automount_service_account_token: None,
            containers,
            dns_config: None,
            dns_policy: None,
            enable_service_links: None,
            host_aliases: None,
            host_ipc: None,
            host_network: None,
            host_pid: None,
            hostname: None,
            image_pull_secrets: None,
            init_containers: Some(vec!(git_checkout_init_container.to_container())),
            node_name: None,
            node_selector: None,
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
            tolerations: None,
            volumes: Some(volumes),
        }),
        status: None
    };

    info!("Pod configuration to deploy: {}", json!(pod_deployment_config));

    pod_deployment_config
}
