use serde_derive::{Deserialize, Serialize};
use reqwest::header::{ACCEPT, USER_AGENT};
use warp::http::StatusCode;
use chrono::Utc;
use log::{info};

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

#[derive(Serialize)]
struct CompletedCheckRunRequest {
    accept: String,
    name: String,
    status: String,
    started_at: String, // ISO 8601
    conclusion: String,
    completed_at: String, // ISO 8601
}

pub struct GithubInstallationClient {
    pub repository_name: String,
    pub github_installation_token: String,
    pub base_url: String,
}

impl GithubInstallationClient {
    pub async fn create_check_run(&self, name: &String, head_sha: &String) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs", self.base_url, self.repository_name);

        let create_check_run_request = CreateCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: name.to_string(),
            head_sha: head_sha.to_string(),
        };

        info!("Creating the check run...");

        let response = reqwest::Client::new()
            .post(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.antiope-preview+json")
            .header(USER_AGENT, "my-test-app")
            .json(&create_check_run_request)
            .send()
            .await?;

        match response.status() {
            StatusCode::CREATED => Ok(()),
            other => Err(other.to_string().into()),
        } 
    }

    pub async fn set_check_run_in_progress(&self, name: &String, check_run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs/{}", self.base_url, self.repository_name, check_run_id);

        let update_check_run_request = UpdateCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: name.to_string(),
            status: "in_progress".to_string(),
            started_at: Utc::now().to_rfc3339(),
        };

        info!("Setting the check run to in progress...");

        let response = reqwest::Client::new()
            .patch(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.antiope-preview+json")
            .header(USER_AGENT, "my-test-app")
            .json(&update_check_run_request)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => Ok(()),
            other => Err(other.to_string().into()),
        } 
    }

    pub async fn set_check_run_complete(&self, name: &String, started_at: String, check_run_id: i64, conclusion: String) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs/{}", self.base_url, self.repository_name, check_run_id);

        let update_check_run_request = CompletedCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: name.to_string(),
            status: "completed".to_string(),
            started_at: started_at,
            completed_at: Utc::now().to_rfc3339(),
            conclusion: conclusion.to_string(),
        };

        info!("Setting the check run to complete!");

        let response = reqwest::Client::new()
            .patch(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.antiope-preview+json")
            .header(USER_AGENT, "my-test-app")
            .json(&update_check_run_request)
            .send()
            .await?;

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
