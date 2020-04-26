use serde_derive::{Serialize,Deserialize};
use reqwest::header::{ACCEPT, USER_AGENT};
use warp::http::StatusCode;
use log::info;
use crate::CompleteCheckRunRequest;

#[derive(Serialize)]
struct CreateCheckRunRequest {
    accept: String,
    name: String,
    head_sha: String
}

#[derive(Serialize)]
struct UpdateCheckRunRequest {
    accept: String,
    name: String,
    status: String,
    started_at: String, // ISO 8601
}

#[derive(Serialize, Debug)]
struct CheckRunOutput {
    title: String,
    summary: String,
    text: String,
}

#[derive(Serialize, Debug)]
struct CompletedCheckRunRequest {
    accept: String,
    name: String,
    status: String,
    started_at: String, // ISO 8601
    conclusion: Option<String>,
    completed_at: Option<String>, // ISO 8601
    output: Option<CheckRunOutput>,
}

pub struct GithubInstallationClient {
    pub repository_name: String,
    pub github_installation_token: String,
    pub base_url: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateCheckRunResponse {
    pub id: i32,
}

impl GithubInstallationClient {
    pub async fn create_check_run(&self, name: &String, head_sha: &String) -> Result<CreateCheckRunResponse, Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs", self.base_url, self.repository_name);

        let create_check_run_request = CreateCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: name.to_string(),
            head_sha: head_sha.to_string(),
        };

        info!("Creating the check run...");

        let check_run_response = reqwest::Client::new()
            .post(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.antiope-preview+json")
            .header(USER_AGENT, "my-test-app")
            .json(&create_check_run_request)
            .send()
            .await?
            .json::<CreateCheckRunResponse>()
            .await?;

        Ok(check_run_response)
    }

    pub async fn set_check_run_complete(&self, update_check_run_request: CompleteCheckRunRequest) -> Result<(), Box<dyn std::error::Error>> {
        let name = update_check_run_request.name.clone();
        
        let request_url = format!("{}/repos/{}/check-runs/{}", self.base_url, self.repository_name, update_check_run_request.check_run_id);

        let update_check_run_request = CompletedCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name,
            status: update_check_run_request.status,
            started_at: update_check_run_request.started_at,
            completed_at: update_check_run_request.finished_at,
            conclusion: update_check_run_request.conclusion,
            output: Some(CheckRunOutput {
                title: update_check_run_request.name.clone(),
                summary: "Complete!".to_string(),
                text: update_check_run_request.logs,
            }),
        };

        info!("Setting the check run to complete with request: {:?}", update_check_run_request);

        let response = reqwest::Client::new()
            .patch(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.antiope-preview+json")
            .header(USER_AGENT, "my-test-app")
            .json(&update_check_run_request)
            .send()
            .await?;

        info!("Response was: {:?}", response);

        match response.status() {
            StatusCode::OK => Ok(()),
            other => Err(other.to_string().into()),
        } 
    }

    pub async fn get_pipeline_file(&self, github_commit_sha: &String) -> Result<String, Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/contents/.kubes-cd/pipeline.yml?ref={}", self.base_url, self.repository_name, github_commit_sha);

        info!("Downloading the steps to run...");

        let response = reqwest::Client::new()
            .get(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.VERSION.raw")
            .header(USER_AGENT, "my-test-app")
            .send()
            .await?
            .text_with_charset("utf-8")
            .await?;

        info!("Got the pipeline file {}", response);

        return Ok(response);
    }
}
