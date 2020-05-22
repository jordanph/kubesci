use crate::github::auth::authenticate_app;
use crate::github::client::auth::GithubAuthorisationClient;
use crate::github::client::installation::GithubInstallationClient;
use crate::routes::CompleteCheckRunRequest;
use chrono::Utc;
use std::convert::Infallible;
use std::env;
use warp::http::StatusCode;

pub async fn handle_update_check_run_request(
    installation_id: u32,
    github_webhook_request: CompleteCheckRunRequest,
) -> Result<impl warp::Reply, Infallible> {
    match update_check_run(installation_id, github_webhook_request).await {
        Ok(()) => Ok(warp::reply::with_status(
            "good shit".to_string(),
            StatusCode::OK,
        )),
        Err(error) => Ok(warp::reply::with_status(
            error.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn update_check_run(
    installation_id: u32,
    update_check_run_request: CompleteCheckRunRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let github_private_key = env::var("GITHUB_APPLICATION_PRIVATE_KEY")?;
    let application_id = env::var("APPLICATION_ID")?;
    let now = Utc::now().timestamp();

    let github_jwt_token = authenticate_app(&github_private_key, &application_id, now)?;

    let github_authorisation_client = GithubAuthorisationClient {
        github_jwt_token,
        base_url: "https://api.github.com".to_string(),
    };

    let installation_access_token = github_authorisation_client
        .get_installation_access_token(installation_id)
        .await?;

    let github_installation_client = GithubInstallationClient {
        repository_name: &update_check_run_request.repo_name,
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    let check_run = github_installation_client
        .get_check_run(update_check_run_request.check_run_id)
        .await?;

    github_installation_client
        .set_check_run_complete(
            update_check_run_request.check_run_id,
            &update_check_run_request,
            &check_run.name,
            &check_run.started_at,
        )
        .await?;

    Ok(())
}