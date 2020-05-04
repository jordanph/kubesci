use warp::{
  filters::BoxedFilter,
  Filter
};
use serde_derive::Deserialize;


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
pub struct Repository {
    pub full_name: String,
}


#[derive(Deserialize)]
pub struct GithubCheckSuiteRequest {
    pub action: String,
    pub check_suite: CheckSuite,
    pub installation: Installation,
    pub repository: Repository
}

#[derive(Deserialize)]
pub struct CompleteCheckRunRequest {
  pub name: String,
  pub repo_name: String,
  pub check_run_id: i32,
  pub status: String,
  pub started_at: String,
  pub finished_at: Option<String>,
  pub logs: String,
  pub conclusion: Option<String>
}

pub fn check_suite_route() -> BoxedFilter<(GithubCheckSuiteRequest, )> {
  let check_suite_header = warp::header::exact("X-GitHub-Event", "check_suite");
  
  warp::post()
    .and(warp::path("webhook"))
    .and(check_suite_header)
    .and(warp::body::json::<GithubCheckSuiteRequest>())
    .boxed()
}

pub fn update_check_run_route() -> BoxedFilter<(u32, CompleteCheckRunRequest)> {
    warp::path!("update-check-run" / u32)
    .and(warp::post())
    .and(warp::body::json::<CompleteCheckRunRequest>())
    .boxed()
}
