use crate::github::auth::authenticate_app;
use chrono::Utc;
use log::info;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_derive::Deserialize;

pub struct GithubAuthorisationClient {
    github_jwt_token: String,
    base_url: String,
}

#[derive(Deserialize)]
struct InstallationAccessTokenResponse {
    token: String,
}

impl GithubAuthorisationClient {
    pub fn new(
        github_private_key: &str,
        application_id: &str,
    ) -> Result<GithubAuthorisationClient, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp();

        let github_jwt_token = authenticate_app(&github_private_key, &application_id, now)?;

        Ok(GithubAuthorisationClient {
            github_jwt_token,
            base_url: "https://api.github.com".to_string(),
        })
    }

    pub async fn get_installation_access_token(
        &self,
        installation_id: u32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let request_url = format!(
            "{}/app/installations/{}/access_tokens",
            self.base_url, installation_id
        );

        info!(
            "Requesting installation access token at {}...",
            &request_url
        );

        let response = reqwest::Client::new()
            .post(&request_url)
            .bearer_auth(self.github_jwt_token.to_string())
            .header(ACCEPT, "application/vnd.github.machine-man-preview+json")
            .header(USER_AGENT, "my-test-app")
            .send()
            .await?;

        info!(
            "Got response code {} back. Trying to decode now...",
            response.status()
        );

        let response_body = response.json::<InstallationAccessTokenResponse>().await?;

        info!("Successfully got the access token!");

        Ok(response_body.token)
    }
}
