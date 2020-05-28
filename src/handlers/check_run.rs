use crate::pipeline::start_step_section;
use crate::routes::GithubCheckRunRequest;
use std::convert::Infallible;
use warp::http::StatusCode;

pub async fn handle_check_run_request(
    github_webhook_request: GithubCheckRunRequest,
) -> Result<impl warp::Reply, Infallible> {
    if github_webhook_request.action != "requested_action" {
        return Ok(warp::reply::with_status("".to_string(), StatusCode::OK));
    }

    let step_section: usize = github_webhook_request
        .requested_action
        .unwrap()
        .identifier
        .parse()
        .unwrap();

    match start_step_section(
        github_webhook_request.installation.id,
        &github_webhook_request.repository.full_name,
        &github_webhook_request.check_run.check_suite.head_sha,
        &github_webhook_request.check_run.check_suite.head_branch,
        step_section,
    )
    .await
    {
        Ok(()) => Ok(warp::reply::with_status("".to_string(), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            error.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
