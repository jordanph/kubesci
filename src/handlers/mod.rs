use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Pod;
use serde_derive::Serialize;

pub mod check_run;
pub mod check_suite;
pub mod pipeline;
pub mod pipelines;
pub mod steps;

#[derive(Serialize, Clone)]
pub struct Run {
    status: Option<String>,
    commit: String,
}

#[derive(Serialize)]
pub struct Pipeline {
    name: String,
    runs: Vec<Run>,
}

#[derive(Serialize)]
pub struct StepStatus {
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    status: String,
}

#[derive(Serialize)]
pub struct Step {
    name: String,
    status: Option<StepStatus>,
}

#[derive(Serialize)]
pub struct ErrorMessage {
    code: u16,
}

pub fn extract_runs(pod: Vec<&Pod>) -> Vec<Run> {
    pod.iter()
        .map(|pod| {
            let status = pod.status.clone().unwrap().phase;
            let commit = pod
                .metadata
                .clone()
                .unwrap()
                .labels
                .unwrap()
                .get("commit")
                .unwrap()
                .clone();

            Run { status, commit }
        })
        .collect()
}
