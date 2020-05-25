use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::api::core::v1::ContainerStateTerminated;
use k8s_openapi::api::core::v1::Container;
use kube::{
    api::{Api, ListParams, Meta, WatchEvent},
    runtime::Informer,
    Client,
};
use std::collections::HashMap;
use futures::{StreamExt, TryStreamExt};
use log::info;

#[derive(Clone)]
struct RunningPod {
  repo_name: String,
  installation_id: i32,
}

// The container information
// pub repo_name: String,
// pub check_run_id: i32,
// pub status: String,
// pub finished_at: Option<String>,
// pub logs: String,
// pub conclusion: Option<String>,

pub async fn poll_pods() -> () {
  let client = Client::try_default().await.unwrap();
  let pods: Api<Pod> = Api::namespaced(client, "kubesci");

  let inf = Informer::new(pods).params(ListParams::default().timeout(10));

  let mut running_pods: HashMap<String, RunningPod> = HashMap::new();

  loop {
      let mut pods = inf.poll().await.unwrap().boxed();

      while let Some(event) = pods.try_next().await.unwrap() {
        handle_pod(event, &mut running_pods);
      }
  }
}

// This function lets the app handle an event from kube
fn handle_pod(ev: WatchEvent<Pod>, running_pods: &mut HashMap<String, RunningPod>) -> Result<(), Box<dyn std::error::Error>> {
  match ev {
      WatchEvent::Added(pod) => {
        let maybe_running_pod = &pod.meta().labels.as_ref().map(|labels| labels.get("installation_id").map(|installation_id| labels.get("repo_name").map(|repo_name| RunningPod {
          installation_id: installation_id.clone().parse().unwrap(), 
          repo_name: repo_name.clone()
        }
        ))).flatten().flatten();
        
        if let Some(running_pod) = maybe_running_pod {
          let pod_name = "some-name".to_string();

          running_pods.insert(pod_name.clone(),running_pod.clone());
        }
      }
      WatchEvent::Modified(pod) => {
        let maybe_pod = running_pods.get(&pod.name());

        if let Some(running_pod) = maybe_pod {
          // Check to see if one of the containers has finished comparing previous state
          let maybe_newly_finished_pods = pod.status
            .map(|status| status.container_statuses)
            .flatten()
            .map(|container_status| container_status.iter()
              .filter_map(|container_status| {
                let maybe_last_state = container_status.last_state.as_ref();
                let maybe_state = container_status.state.as_ref();

                match (maybe_last_state, maybe_state) {
                  (Some(last_state), Some(state)) => if last_state == state {
                    None
                  } else {
                    if let Some(terminated) = state.terminated.clone() {
                      Some((container_status.name.clone(), terminated))
                    } else {
                      None
                    }
                  },
                  (_, Some(state)) => if let Some(terminated) = state.terminated.clone() {
                    Some((container_status.name.clone(), terminated))
                  } else {
                    None
                  },
                  _ => None,
                }

              }
              ).collect::<Vec<(String, ContainerStateTerminated)>>()
            );

          if let Some(newly_finished_pods) = maybe_newly_finished_pods {
            for (finished_pod_name, finished_pod_state) in newly_finished_pods {
              if let Some(container) = pod.spec.as_ref().map(|pod_spec| pod_spec.containers.as_ref()).map(|containers: &Vec<Container>| containers.iter().find(|container| container.name == finished_pod_name)).flatten() {
                // Get the logs for the container
                // Set the check run to finsihed
              }
            }

          }

          // Check if all pods are now finished
          // Kick of next section
          // Delete the pod
        }
      }
      WatchEvent::Deleted(pod) => {
        // Remove from the current running pods
        running_pods.remove(&pod.name());
        info!("Pod was deleted! {:?}", pod);
      }
      WatchEvent::Bookmark(_) => { }
      WatchEvent::Error(e) => {
      }
  }
  Ok(())
}