use serde_derive::Deserialize;
use warp::{filters::BoxedFilter, Filter};

#[derive(Deserialize)]
pub struct CheckSuite {
    pub head_sha: String,
    pub head_branch: String,
}

#[derive(Deserialize)]
pub struct CheckRun {
    pub id: i64,
    pub check_suite: CheckSuite,
    pub started_at: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct Installation {
    pub id: u32,
}

#[derive(Deserialize)]
pub struct RequestedAction {
    pub identifier: String,
}

#[derive(Deserialize)]
pub struct Repository {
    pub full_name: String,
}

#[derive(Deserialize)]
pub struct GithubCheckSuiteRequest {
    pub action: String,
    pub check_suite: CheckSuite,
    pub installation: Installation,
    pub repository: Repository,
}

#[derive(Deserialize)]
pub struct GithubCheckRunRequest {
    pub action: String,
    pub check_run: CheckRun,
    pub installation: Installation,
    pub repository: Repository,
    pub requested_action: Option<RequestedAction>,
}

#[derive(Deserialize)]
pub struct CompleteCheckRunRequest {
    pub repo_name: String,
    pub check_run_id: i32,
    pub status: String,
    pub finished_at: Option<String>,
    pub logs: String,
    pub conclusion: Option<String>,
}

#[derive(Deserialize)]
pub struct PodSuccessfullyFinishedRequest {
    pub step_section: usize,
    pub repo_name: String,
    pub branch_name: String,
    pub commit_sha: String,
}

pub fn check_suite_route() -> BoxedFilter<(GithubCheckSuiteRequest,)> {
    let check_suite_header = warp::header::exact("X-GitHub-Event", "check_suite");

    warp::post()
        .and(warp::path("webhook"))
        .and(check_suite_header)
        .and(warp::body::json::<GithubCheckSuiteRequest>())
        .boxed()
}

pub fn check_run_route() -> BoxedFilter<(GithubCheckRunRequest,)> {
    let check_run_header = warp::header::exact("X-GitHub-Event", "check_run");

    warp::post()
        .and(warp::path("webhook"))
        .and(check_run_header)
        .and(warp::body::json::<GithubCheckRunRequest>())
        .boxed()
}

pub fn get_pipelines_route() -> BoxedFilter<()> {
    warp::get().and(warp::path("pipelines")).boxed()
}

pub fn get_pipeline_route() -> BoxedFilter<(String,)> {
    warp::path!("pipelines" / String).boxed()
}

pub fn get_pipeline_steps_route() -> BoxedFilter<(String, String)> {
    warp::path!("pipelines" / String / String).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn check_suite_test_handler(
        _check_suite_request: GithubCheckSuiteRequest,
    ) -> std::result::Result<impl warp::reply::Reply, warp::Rejection> {
        Ok(warp::reply())
    }

    #[tokio::test]
    async fn should_respond_to_check_suite_request() {
        let route = check_suite_route().and_then(check_suite_test_handler);

        let body = json!({
            "action": "complete",
            "check_suite": {
                "head_sha": "asnkqf1",
                "head_branch": "test"
            },
            "installation": {
                "id": 12345
            },
            "repository": {
                "full_name": "test-repo"
            }
        });

        let response = warp::test::request()
            .method("POST")
            .path("/webhook")
            .header("X-GitHub-Event", "check_suite")
            .json(&body)
            .reply(&route)
            .await;

        assert_eq!(response.status(), 200)
    }

    #[tokio::test]
    async fn should_respond_with_bad_request_if_check_suite_request_not_in_body() {
        let route = check_suite_route().and_then(check_suite_test_handler);

        let response = warp::test::request()
            .method("POST")
            .path("/webhook")
            .header("X-GitHub-Event", "check_suite")
            .reply(&route)
            .await;

        assert_eq!(response.status(), 400)
    }

    #[tokio::test]
    async fn should_respond_with_bad_request_if_no_check_suite_header() {
        let route = check_suite_route().and_then(check_suite_test_handler);

        let body = json!({
            "action": "complete",
            "check_suite": {
                "head_sha": "asnkqf1",
                "head_branch": "test"
            },
            "installation": {
                "id": 12345
            },
            "repository": {
                "full_name": "test-repo"
            }
        });

        let response = warp::test::request()
            .method("POST")
            .path("/webhook")
            .json(&body)
            .reply(&route)
            .await;

        assert_eq!(response.status(), 400)
    }
}
