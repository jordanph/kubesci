use serde_derive::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::Filter;
use chrono::{DateTime, Utc, Duration};
use std::env;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::header::{AUTHORIZATION, ACCEPT};
use warp::http::StatusCode;

#[derive(Deserialize, Serialize)]
struct GithubWebhook {
    action: String,
}

#[tokio::main]
async fn main() {
    let promote = warp::post()
        .and(warp::path("test"))
        // .and(warp::body::json())
        .and_then(handle_request);

    warp::serve(promote).run(([127, 0, 0, 1], 3030)).await
}

async fn handle_request() -> Result<impl warp::Reply, Infallible> {
    match do_stuff().await {
        Ok(()) => Ok(warp::reply::with_status("good shit".to_string(), StatusCode::OK)),
        Err(_) => Ok(warp::reply::with_status("".to_string(), StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

async fn do_stuff() -> Result<(), Box<dyn std::error::Error>> {
    let github_jwt_token = authenticate_app()?;

    let github_authorisation_client = GithubAuthorisationClient {
        github_jwt_token: github_jwt_token,
        base_url: "https://api.github.com".to_string(),
    };

    let installation_access_token = github_authorisation_client.get_installation_access_token("1234".to_string()).await?;

    let github_installation_client = GithubInstallationClient {
        repository_name: "test_123".to_string(),
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    github_installation_client.create_check_run("12345".to_string()).await?;

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

impl GithubInstallationClient {
    async fn create_check_run(&self, head_sha: String) -> Result<(), Box<dyn std::error::Error>> {
        let request_url = format!("{}/repos/{}/check-runs", self.base_url, self.repository_name);

        let create_check_run_request = CreateCheckRunRequest {
            accept: "application/vnd.github.antiope-preview+json".to_string(),
            name: "Test run".to_string(),
            head_sha: head_sha
        };

        let bearer_header = format!("Bearer {}", self.github_installation_token);

        reqwest::Client::new()
            .post(&request_url)
            .header(AUTHORIZATION, bearer_header)
            .json(&create_check_run_request)
            .send()
            .await?;
    
        Ok(())
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
    async fn get_installation_access_token(&self, installation_id: String) -> Result<String, Box<dyn std::error::Error>> {
        let request_url = format!("{}/app/installations/{}/access_tokens", self.base_url, installation_id);

        let bearer_header = format!("Bearer {}", self.github_jwt_token);

        let response = reqwest::Client::new()
            .get(&request_url)
            .header(AUTHORIZATION, bearer_header)
            .header(ACCEPT, "application/vnd.github.machine-man-preview+json")
            .send()
            .await?
            .json::<InstallationAccessTokenResponse>()
            .await?;
    
        Ok(response.token)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    exp: DateTime<Utc>, // Required (validate_exp defaults to true in validation). Expiration time
    iat: DateTime<Utc>, // Optional. Issued at
    iss: String         // Optional. Issuer
}

fn authenticate_app() -> Result<std::string::String, Box<dyn std::error::Error>> {
    let application_id = env::var("APPLICATION_ID")?;

    let claim = Claims {
        exp: Utc::now(),
        iat: Utc::now() + Duration::minutes(10),
        iss: application_id
    };

    let secret = env::var("PRIVATE_KEY")?;

    let token = encode(&Header::new(Algorithm::RS256), &claim, &EncodingKey::from_rsa_pem(secret.as_bytes())?)?;

    return Ok(token);
}