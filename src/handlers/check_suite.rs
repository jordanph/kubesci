use crate::pipeline::PipelineService;
use crate::routes::GithubCheckSuiteRequest;
use std::convert::Infallible;
use warp::http::StatusCode;

pub async fn handle_check_suite_request(
    github_webhook_request: GithubCheckSuiteRequest,
    pipeline_service: PipelineService,
) -> Result<impl warp::Reply, Infallible> {
    if github_webhook_request.action != "requested" {
        return Ok(warp::reply::with_status("".to_string(), StatusCode::OK));
    }

    match pipeline_service
        .start_step_section(
            github_webhook_request.installation.id,
            &github_webhook_request.repository.full_name,
            &github_webhook_request.check_suite.head_sha,
            &github_webhook_request.check_suite.head_branch,
            None,
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
