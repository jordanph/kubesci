use crate::routes::CompleteCheckRunRequest;
use log::info;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_derive::{Deserialize, Serialize};
use warp::http::StatusCode;

#[derive(Serialize)]
struct CreateCheckRunRequest {
    accept: String,
    name: String,
    head_sha: String,
}

#[derive(Serialize)]
struct UpdateCheckRunRequest {
    accept: String,
    name: String,
    status: String,
    started_at: String, // ISO 8601
}

#[derive(Serialize, Debug)]
struct CheckRunOutput<'a> {
    title: &'a str,
    summary: &'a str,
    text: &'a str,
}

#[derive(Serialize, Debug)]
struct CompletedCheckRunRequest<'a> {
    accept: &'a str,
    name: &'a str,
    status: &'a str,
    started_at: &'a str, // ISO 8601
    conclusion: &'a Option<String>,
    completed_at: &'a Option<String>, // ISO 8601
    output: Option<&'a CheckRunOutput<'a>>,
}

#[derive(Deserialize, Debug)]
pub struct GetCheckRunResponse {
    pub name: String,
    pub started_at: String
}

pub struct GithubInstallationClient {
    pub repository_name: String,
    pub github_installation_token: String,
    pub base_url: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateCheckRunResponse {
    pub id: u32,
}

impl GithubInstallationClient {
    pub async fn create_check_run(
        &self,
        name: &str,
        head_sha: &str,
    ) -> Result<CreateCheckRunResponse, Box<dyn std::error::Error>> {
        let request_url = format!(
            "{}/repos/{}/check-runs",
            self.base_url, self.repository_name
        );

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

    pub async fn get_check_run(
        &self,
        check_run_id: &i32
    ) -> Result<GetCheckRunResponse, Box<dyn std::error::Error>> {
        let request_url = format!(
            "{}/repos/{}/check-runs/{}",
            self.base_url, self.repository_name, check_run_id,
        );

        info!("Getting the check run {}...", check_run_id);

        let check_run_response = reqwest::Client::new()
            .get(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.antiope-preview+json")
            .header(USER_AGENT, "my-test-app")
            .send()
            .await?
            .json::<GetCheckRunResponse>()
            .await?;

        Ok(check_run_response)
    }

    pub async fn set_check_run_complete(
        &self,
        check_run_id: &i32,
        update_check_run_request: &CompleteCheckRunRequest,
        name: &str,
        started_at: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!(
            "{}/repos/{}/check-runs/{}",
            self.base_url, self.repository_name, check_run_id
        );

        let check_run_output = CheckRunOutput {
            title: name,
            summary: "Complete!",
            text: &update_check_run_request.logs,
        };

        let update_check_run_request = CompletedCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json",
            name: name,
            status: &update_check_run_request.status,
            started_at: &started_at.to_string(),
            completed_at: &update_check_run_request.finished_at,
            conclusion: &update_check_run_request.conclusion,
            output: Some(&check_run_output),
        };

        info!(
            "Setting the check run to complete with request: {:?}",
            update_check_run_request
        );

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

    pub async fn get_pipeline_file(
        &self,
        github_commit_sha: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let request_url = format!(
            "{}/repos/{}/contents/.kubes-cd/pipeline.yml?ref={}",
            self.base_url, self.repository_name, github_commit_sha
        );

        info!("Downloading the steps to run...");

        let response = reqwest::Client::new()
            .get(&request_url)
            .bearer_auth(self.github_installation_token.to_string())
            .header(ACCEPT, "application/vnd.github.VERSION.raw")
            .header(USER_AGENT, "my-test-app")
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => Ok(Some(response.text_with_charset("utf-8").await?)),
            StatusCode::NOT_FOUND => Ok(None),
            _ => Err("Error!".into()),
        }
    }
}
