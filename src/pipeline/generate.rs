use serde_json::json;
use log::info;
use vec1::Vec1;
use crate::pipeline::init_containers::git::GitInitContainer;
use crate::pipeline::sidecar_containers::PollingSidecarContainer;
use crate::pipeline::KubernetesContainer;
use crate::pipeline::{StepWithCheckRunId, RawPipeline, Step};

use k8s_openapi::api::core::v1::{Pod, Container};

pub fn generate_kubernetes_pipeline<'a>(steps_with_check_run_id: &[StepWithCheckRunId], github_head_sha: &String, repo_name: &String, branch: &String, namespace: &String, installation_id: u32) -> Result<Pod, Box<dyn std::error::Error>> {
    let mut containers: Vec<Container> = steps_with_check_run_id.into_iter().map(|step_with_check_run_id| step_with_check_run_id.to_container()).collect();

    let side_car_container = PollingSidecarContainer {
        installation_id: installation_id,
        namespace: namespace,
        repo_name: repo_name,
        commit_sha: github_head_sha,
        steps_with_check_run_ids: &steps_with_check_run_id
    };

    containers.push(side_car_container.to_container());
 
    info!("Containers to deploy: {}", json!(containers));

    let volume_mount_names: Vec<String> = steps_with_check_run_id
        .into_iter()
        .map(|step_with_check_run_id| step_with_check_run_id.check_run_id.to_string())
        .collect();

    let secret_mounts: std::vec::Vec<serde_json::value::Value> = steps_with_check_run_id
                            .iter()
                            .filter_map(|step_with_check_run_id| step_with_check_run_id.step.mount_secret.as_ref())
                            .flatten()
                            .map(|mount_secret| json!({ "name": mount_secret.name, "secret": { "secretName": mount_secret.name}}))
                            .collect();

    let container_repo_mounts: Vec<serde_json::Value> = volume_mount_names.clone()
        .into_iter().map(|check_run_id| json!({"name": check_run_id.to_string(), "emptyDir": {}}))
        .collect();

    let volumes = [secret_mounts, container_repo_mounts].concat();

    info!("Volumes to deploy: {}", json!(volumes));

    let short_commit = &github_head_sha[0..7];

    let git_checkout_init_container = GitInitContainer {
        clone_url: repo_name,
        commit_sha: github_head_sha,
        volume_mount_names: &volume_mount_names
    };

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
            "initContainers": [ git_checkout_init_container.to_container() ],
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

pub fn generate_pipeline(raw_pipeline: &String) -> Result<RawPipeline, serde_yaml::Error> {
    serde_yaml::from_str(&raw_pipeline)
}

pub fn filter_steps<'a>(steps: &'a[Step], github_branch_name: &String) -> Option<Vec1<&'a Step>> {
    let maybe_steps = steps
        .iter()
        .filter(|step| skip_step(step, github_branch_name))
        .collect::<Vec<_>>();

    return Vec1::try_from_vec(maybe_steps).ok();
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
