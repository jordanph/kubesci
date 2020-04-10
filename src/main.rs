use serde_derive::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::Filter;
use chrono::{Utc};
use std::env;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::header::{ACCEPT, USER_AGENT};
use warp::http::StatusCode;
use log::{info};
use k8s_openapi::api::core::v1::Pod;
use serde_json::json;
use std::{thread, time};

use kube::{
    api::{Api, Meta, PostParams},
    Client,
};

#[derive(Deserialize)]
struct CheckSuite {
    head_sha: String,
}

#[derive(Deserialize)]
struct CheckRun {
    id: i64,
}

#[derive(Deserialize)]
struct Installation {
    id: u32,
}

#[derive(Deserialize)]
struct Repository {
    full_name: String,
}

#[derive(Deserialize)]
struct GithubCheckSuiteRequest {
    action: String,
    check_suite: CheckSuite,
    installation: Installation,
    repository: Repository
}

#[derive(Deserialize)]
struct GithubCheckRunRequest {
    action: String,
    check_run: CheckRun,
    installation: Installation,
    repository: Repository
}

#[tokio::main]
async fn main() {
    let _ = pretty_env_logger::try_init();

    let check_suite_header = warp::header::exact("X-GitHub-Event", "check_suite");

    let check_suite_handler = warp::post()
        .and(warp::path("test"))
        .and(check_suite_header)
        .and(warp::body::json::<GithubCheckSuiteRequest>())
        .and_then(handle_check_suite_request);

    let check_suite_header = warp::header::exact("X-GitHub-Event", "check_run");

    let check_run_handler = warp::post()
        .and(warp::path("test"))
        .and(check_suite_header)
        .and(warp::body::json::<GithubCheckRunRequest>())
        .and_then(handle_check_run_request);

    warp::serve(check_suite_handler.or(check_run_handler)).run(([127, 0, 0, 1], 3030)).await
}

async fn handle_check_run_request(github_webhook_request: GithubCheckRunRequest) -> Result<impl warp::Reply, Infallible> {
    if github_webhook_request.action == "completed" {
        return Ok(warp::reply::with_status("good shit".to_string(), StatusCode::OK));
    }

    match set_check_run_in_progress(github_webhook_request).await {
        Ok(()) => Ok(warp::reply::with_status("good shit".to_string(), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(error.to_string(), StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

async fn handle_check_suite_request(github_webhook_request: GithubCheckSuiteRequest) -> Result<impl warp::Reply, Infallible> {
    if github_webhook_request.action == "completed" {
        return Ok(warp::reply::with_status("good shit".to_string(), StatusCode::OK));
    }

    match create_check_run(github_webhook_request).await {
        Ok(()) => Ok(warp::reply::with_status("good shit".to_string(), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(error.to_string(), StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

async fn set_check_run_in_progress(github_webhook_request: GithubCheckRunRequest) -> Result<(), Box<dyn std::error::Error>> {
    info!("Setting up CI process for check run {}..", github_webhook_request.check_run.id);
    
    let client = Client::infer().await?;
    let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

    // Manage pods
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    
    info!("Creating Pod instance blog...");
    
    let pod_deployment_config: Pod = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "blog" },
        "spec": {
            "containers": [{
              "name": "myapp-container-1",
              "image": "busybox",
              "command": ["sh", "-c", "echo Hello Kubernetes! && sleep 20"]
            },
            ],
            "restartPolicy": "Never",
        }
    }))?;

    let pp = PostParams::default();
    match pods.create(&pp, &pod_deployment_config).await {
        Ok(o) => {
            let name = Meta::name(&o);
            info!("Created pod: {}!", name);
        }
        Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
        Err(e) => return Err(e.into()),                        // any other case is probably bad
    }

    info!("Pod has spun up! Notify Github that test is in progress...");

    let github_jwt_token = authenticate_app()?;

    let github_authorisation_client = GithubAuthorisationClient {
        github_jwt_token: github_jwt_token,
        base_url: "https://api.github.com".to_string(),
    };

    let installation_access_token = github_authorisation_client.get_installation_access_token(github_webhook_request.installation.id).await?;

    let github_installation_client = GithubInstallationClient {
        repository_name: github_webhook_request.repository.full_name.to_string(),
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    github_installation_client.set_check_run_in_progress(github_webhook_request.check_run.id).await?;

    info!("Waiting for pod to finish to return completion state...");

    let termination_status = loop {
        let p1cpy = pods.get("blog").await?;
        if let Some(status) = p1cpy.status {
            if let Some(container_statuses) = status.container_statuses {
                let maybe_container_status = container_statuses
                    .into_iter()
                    .find(|container_status| container_status.name == "myapp-container-1");

                if let Some(container_status) = maybe_container_status {
                    if let Some(state) = container_status.state {
                        if let Some(_) = state.running {
                            info!("Pod is still running...");
                        } else if let Some(terminated) = state.terminated {
                            info!("Pod has finished!");
                            break terminated;
                        } else {
                            info!("Pod is still waiting...");
                        }
                    }
                }

            }
        }

        thread::sleep(time::Duration::from_secs(5));
    };

    if termination_status.exit_code == 0 {
        github_installation_client.set_check_run_complete(github_webhook_request.check_run.id, "success".to_string()).await?;
    } else {
        github_installation_client.set_check_run_complete(github_webhook_request.check_run.id, "failure".to_string()).await?;
    }

    Ok(())
}

async fn create_check_run(github_webhook_request: GithubCheckSuiteRequest) -> Result<(), Box<dyn std::error::Error>> {
    let github_jwt_token = authenticate_app()?;

    let github_authorisation_client = GithubAuthorisationClient {
        github_jwt_token: github_jwt_token,
        base_url: "https://api.github.com".to_string(),
    };

    let installation_access_token = github_authorisation_client.get_installation_access_token(github_webhook_request.installation.id).await?;

    let github_installation_client = GithubInstallationClient {
        repository_name: github_webhook_request.repository.full_name.to_string(),
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    github_installation_client.create_check_run(github_webhook_request.check_suite.head_sha).await?;

    Ok(())
}

struct GithubInstallationClient {
    repository_name: String,
    github_installation_token: String,
    base_url: String,
}

#[derive(Deserialize, Serialize)]
struct CreateCheckRunRequest {
    accept: String,
    name: String,
    head_sha: String
}

#[derive(Deserialize, Serialize)]
struct UpdateCheckRunRequest {
    accept: String,
    name: String,
    status: String,
    started_at: String, // ISO 8601
}

#[derive(Deserialize, Serialize)]
struct CompletedCheckRunRequest {
    accept: String,
    name: String,
    status: String,
    started_at: String, // ISO 8601
    conclusion: String,
    completed_at: String, // ISO 8601
}


impl GithubInstallationClient {
    async fn create_check_run(&self, head_sha: String) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs", self.base_url, self.repository_name);

        let create_check_run_request = CreateCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: "Test run".to_string(),
            head_sha: head_sha
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

    async fn set_check_run_in_progress(&self, check_run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs/{}", self.base_url, self.repository_name, check_run_id);

        let update_check_run_request = UpdateCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: "Test run".to_string(),
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

    async fn set_check_run_complete(&self, check_run_id: i64, conclusion: String) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs/{}", self.base_url, self.repository_name, check_run_id);

        let update_check_run_request = CompletedCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: "Test run".to_string(),
            status: "completed".to_string(),
            started_at: Utc::now().to_rfc3339(),
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
}

struct GithubAuthorisationClient {
    github_jwt_token: String,
    base_url: String,
}

#[derive(Deserialize)]
struct InstallationAccessTokenResponse {
    token: String,
}

impl GithubAuthorisationClient {
    async fn get_installation_access_token(&self, installation_id: u32) -> Result<String, Box<dyn std::error::Error>> {
        let request_url = format!("{}/app/installations/{}/access_tokens", self.base_url, installation_id);

        info!("Requesting installation access token at {}...", &request_url);

        let response = reqwest::Client::new()
            .post(&request_url)
            .bearer_auth(self.github_jwt_token.to_string())
            .header(ACCEPT, "application/vnd.github.machine-man-preview+json")
            .header(USER_AGENT, "my-test-app")
            .send()
            .await?;

        info!("Got response code {} back. Trying to decode now...", response.status());

        let response_body = response
            .json::<InstallationAccessTokenResponse>()
            .await?;
    
        info!("Successfully got the access token!");

        Ok(response_body.token)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    exp: i64, // Required (validate_exp defaults to true in validation). Expiration time
    iat: i64, // Optional. Issued at
    iss: String         // Optional. Issuer
}

fn authenticate_app() -> Result<std::string::String, Box<dyn std::error::Error>> {
    let application_id = env::var("APPLICATION_ID")?;

    let now = Utc::now().timestamp();
    let ten_minutes_from_now = now + (10 * 60);

    let claim = Claims {
        exp: ten_minutes_from_now,
        iat: now,
        iss: application_id
    };

    let secret = env::var("PRIVATE_KEY")?;

    let token = encode(&Header::new(Algorithm::RS256), &claim, &EncodingKey::from_rsa_pem(secret.as_bytes())?)?;

    return Ok(token);
}
