use warp::Filter;
use serde_derive::Deserialize;

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
pub struct GithubCheckSuiteRequest {
    action: String,
    check_suite: CheckSuite,
    installation: Installation,
    repository: Repository
}

#[derive(Deserialize)]
pub struct GithubCheckRunRequest {
    action: String,
    check_run: CheckRun,
    installation: Installation,
    repository: Repository
}

pub fn app_routes() -> warp::filter::FilterBase {
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

    check_suite_handler.or(check_run_handler);
}