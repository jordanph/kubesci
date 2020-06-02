use k8s_openapi::api::core::v1::{ContainerState, ContainerStateTerminated, Pod};

pub fn extract_newly_finished_container_states(
    pod: &Pod,
) -> Option<Vec<(String, ContainerStateTerminated)>> {
    pod.status
        .as_ref()
        .map(|status| status.container_statuses.as_ref())
        .flatten()
        .map(|container_status| {
            container_status
                .iter()
                .filter_map(|container_status| {
                    let maybe_last_state = container_status.last_state.as_ref();
                    let maybe_state = container_status.state.as_ref();

                    let maybe_terminated_state =
                        extract_terminated_state(maybe_last_state, maybe_state);

                    maybe_terminated_state
                        .map(|terminated_state| (container_status.name.clone(), terminated_state))
                })
                .collect::<Vec<(String, ContainerStateTerminated)>>()
        })
}

fn extract_terminated_state(
    last_state: Option<&ContainerState>,
    current_state: Option<&ContainerState>,
) -> Option<ContainerStateTerminated> {
    match (last_state, current_state) {
        (Some(last_state), Some(state)) => {
            if last_state.terminated.is_some() {
                None
            } else if let Some(terminated) = state.terminated.clone() {
                Some(terminated)
            } else {
                None
            }
        }
        (None, Some(state)) => {
            if let Some(terminated) = state.terminated.clone() {
                Some(terminated)
            } else {
                None
            }
        }
        _ => None,
    }
}
