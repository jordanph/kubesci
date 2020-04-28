use serde_derive::{Deserialize,Serialize};
use std::convert::Infallible;
use warp::Filter;
use warp::http::StatusCode;
use log::{info, error};
use k8s_openapi::api::core::v1::Pod;
use std::net::SocketAddr;

#[macro_use]
extern crate vec1;

use github::client::auth::GithubAuthorisationClient;
use github::client::installation::GithubInstallationClient;
use github::auth::authenticate_app;

use handlers::{get_pipelines, get_pipeline, get_pipeline_steps};

mod github;
mod pipeline;
mod handlers;

use kube::{
    api::{Api, Meta, PostParams},
    Client,
};

#[derive(Deserialize)]
struct CheckSuite {
    head_sha: String,
    head_branch: String,
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
pub struct CompleteCheckRunRequest {
  name: String,
  repo_name: String,
  check_run_id: i32,
  status: String,
  started_at: String,
  finished_at: Option<String>,
  logs: String,
  conclusion: Option<String>
}

#[tokio::main]
async fn main() {
    let _ = pretty_env_logger::try_init();

    let cors = warp::cors()
        .allow_origin("http://localhost:3000");

    let check_suite_header = warp::header::exact("X-GitHub-Event", "check_suite");

    let check_suite_handler = warp::post()
        .and(warp::path("webhook"))
        .and(check_suite_header)
        .and(warp::body::json::<GithubCheckSuiteRequest>())
        .and_then(handle_check_suite_request);

    let update_check_run_handler = warp::path!("update-check-run" / u32)
        .and(warp::post())
        .and(warp::body::json::<CompleteCheckRunRequest>())
        .and_then(handle_update_check_run_request);

    let get_pipelines_handler = warp::get()
        .and(warp::path("pipelines"))
        .and_then(handle_get_pipelines)
        .with(cors);

    let pipeline_cors = warp::cors()
        .allow_origin("http://localhost:3000");

    let get_pipeline_handler = warp::path!("pipelines" / String)
        .and_then(handle_get_pipeline)
        .with(pipeline_cors);

    let steps_cors = warp::cors()
        .allow_origin("http://localhost:3000");

    let get_pipeline_steps_handler = warp::path!("pipelines" / String / String)
        .and_then(handle_get_steps)
        .with(steps_cors);

    let app_routes = check_suite_handler.or(update_check_run_handler).or(get_pipeline_steps_handler).or(get_pipeline_handler).or(get_pipelines_handler);

    let address = std::env::var("SOCKET_ADDRESS").unwrap_or("127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or("3030".to_string());

    let socket_address: SocketAddr = format!("{}:{}", address, port).parse().expect("Unable to parse socket address");

    warp::serve(app_routes).run(socket_address).await
}

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
}

async fn handle_get_pipelines() -> Result<impl warp::Reply, Infallible> {
    match get_pipelines().await {
        Ok(pods) => {
            let json = warp::reply::json(&pods);

            Ok(warp::reply::with_status(json, StatusCode::OK))
        },
        Err(error) => {
            error!("Unexpected error occurred: {}", error);

            let json = warp::reply::json(&ErrorMessage {
                code: 500
            });

            Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
        },
    }
}

async fn handle_get_pipeline(pipeline_name: String) -> Result<impl warp::Reply, Infallible> {
    match get_pipeline(pipeline_name).await {
        Ok(pods) => {
            let json = warp::reply::json(&pods);

            Ok(warp::reply::with_status(json, StatusCode::OK))
        },
        Err(error) => {
            error!("Unexpected error occurred: {}", error);

            let json = warp::reply::json(&ErrorMessage {
                code: 500
            });

            Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
        },
    }
}

async fn handle_get_steps(pipeline_name: String, commit: String) -> Result<impl warp::Reply, Infallible> {
    match get_pipeline_steps(pipeline_name, commit).await {
        Ok(pods) => {
            let json = warp::reply::json(&pods);

            Ok(warp::reply::with_status(json, StatusCode::OK))
        },
        Err(error) => {
            error!("Unexpected error occurred: {}", error);

            let json = warp::reply::json(&ErrorMessage {
                code: 500
            });

            Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
        },
    }
}

async fn handle_update_check_run_request(installation_id: u32, github_webhook_request: CompleteCheckRunRequest) -> Result<impl warp::Reply, Infallible> {
    match update_check_run(installation_id, github_webhook_request).await {
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

async fn update_check_run(installation_id: u32, update_check_run_request: CompleteCheckRunRequest) -> Result<(), Box<dyn std::error::Error>> {
    let github_jwt_token = authenticate_app()?;

    let github_authorisation_client = GithubAuthorisationClient {
        github_jwt_token: github_jwt_token,
        base_url: "https://api.github.com".to_string(),
    };

    let installation_access_token = github_authorisation_client.get_installation_access_token(installation_id).await?;

    let github_installation_client = GithubInstallationClient {
        repository_name: update_check_run_request.repo_name.clone(),
        github_installation_token: installation_access_token,
        base_url: "https://api.github.com".to_string(),
    };

    github_installation_client.set_check_run_complete(update_check_run_request).await?;

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

    let raw_pipeline = github_installation_client.get_pipeline_file(&github_webhook_request.check_suite.head_sha).await?;

    let pipeline = pipeline::generate::generate_pipeline(&raw_pipeline)?;

    let maybe_steps = pipeline::generate::filter_steps(&pipeline.steps, &github_webhook_request.check_suite.head_branch);

    if let Some(steps) = maybe_steps {
        let mut check_run_ids: Vec<(String, i32)> = Vec::new();
 
        for step in &steps {
            let checkrun_response = github_installation_client.create_check_run(&step.name, &github_webhook_request.check_suite.head_sha).await?;

            check_run_ids.push((step.name.replace(" ", "-").to_lowercase(), checkrun_response.id));
        }

        let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

        let pod_deployment = pipeline::generate::generate_kubernetes_pipeline(
            &steps,
            &github_webhook_request.check_suite.head_sha,
            &github_webhook_request.repository.full_name,
            &github_webhook_request.check_suite.head_branch,
            check_run_ids,
            &namespace,
            github_webhook_request.installation.id
        )?;

        let client = Client::infer().await?;

        let pods: Api<Pod> = Api::namespaced(client, &namespace);

        info!("Creating Pod for checks...");

        let pp = PostParams::default();
        match pods.create(&pp, &pod_deployment).await {
            Ok(o) => {
                let name = Meta::name(&o);
                info!("Created pod: {}!", name);
            }
            Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409),
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}
