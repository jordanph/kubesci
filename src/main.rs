use serde_derive::{Deserialize};
use std::convert::Infallible;
use warp::Filter;
use warp::http::StatusCode;
use log::{info};
use k8s_openapi::api::core::v1::Pod;
use std::time;

use github::client::auth::GithubAuthorisationClient;
use github::client::installation::GithubInstallationClient;
use github::auth::authenticate_app;

mod github;
mod pipeline;

use kube::{
    api::{Api, Meta, PostParams, LogParams},
    Client,
};

#[derive(Deserialize)]
struct CheckSuite {
    head_sha: String,
}

#[derive(Deserialize)]
struct CheckRun {
    id: i64,
    check_suite: CheckSuite,
    started_at: String,
    name: String,
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

    let app_routes = check_suite_handler.or(check_run_handler);

    warp::serve(app_routes).run(([127, 0, 0, 1], 3030)).await
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

    let pods: Api<Pod> = Api::namespaced(client, &namespace);

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

    info!("Waiting for pod to finish to return completion state...");

    let mut set_in_progress = false;

    let termination_status = loop {
        let p1cpy = pods.get(&github_webhook_request.check_run.check_suite.head_sha).await?;

        if let Some(status) = p1cpy.status {
            if let Some(container_statuses) = status.container_statuses {
                let maybe_container_status = container_statuses
                    .into_iter()
                    .find(|container_status| container_status.name == github_webhook_request.check_run.name.replace(" ", "-").to_lowercase());

                if let Some(container_status) = maybe_container_status {
                    if let Some(state) = container_status.state {
                        if let Some(_) = state.running {
                            if !set_in_progress {
                                info!("Container for has spun up! Notify Github that test is in progress...");

                                github_installation_client.set_check_run_in_progress(&github_webhook_request.check_run.name, github_webhook_request.check_run.id).await?;

                                set_in_progress = true;
                            }

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

        tokio::time::delay_for(time::Duration::from_secs(5)).await;
    };

    let mut lp = LogParams::default();
    lp.follow = true;
    lp.timestamps = true;
    lp.container = Some(github_webhook_request.check_run.name.replace(" ", "-").to_lowercase());

    let logs = pods.logs(&github_webhook_request.check_run.check_suite.head_sha, &lp).await?;
    let logs_with_exit_status = "```\n".to_string() + &logs + &format!("\nExited with Status Code: {}", termination_status.exit_code) + &"\n```".to_string();

    if termination_status.exit_code == 0 {
        github_installation_client.set_check_run_complete(&github_webhook_request.check_run.name, github_webhook_request.check_run.started_at, github_webhook_request.check_run.id, "success".to_string(), &logs_with_exit_status).await?;
    } else {
        github_installation_client.set_check_run_complete(&github_webhook_request.check_run.name, github_webhook_request.check_run.started_at, github_webhook_request.check_run.id, "failure".to_string(), &logs_with_exit_status).await?;
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

    let raw_pipeline = github_installation_client.get_pipeline_file(&github_webhook_request.check_suite.head_sha).await?;

    let pipeline = pipeline::generate::generate_pipeline(&raw_pipeline)?;

    let pod_deployment = pipeline::generate::generate_kubernetes_pipeline(
        &pipeline,
        &github_webhook_request.check_suite.head_sha,
        &github_webhook_request.repository.full_name,
    )?;

    let client = Client::infer().await?;
    let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

    let pods: Api<Pod> = Api::namespaced(client, &namespace);

    info!("Creating Pod for checks...");

    let pp = PostParams::default();
    match pods.create(&pp, &pod_deployment).await {
        Ok(o) => {
            let name = Meta::name(&o);
            info!("Created pod: {}!", name);
        }
        Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
        Err(e) => return Err(e.into()),                        // any other case is probably bad
    }

    for step in pipeline.steps {
        github_installation_client.create_check_run(&step.name, &github_webhook_request.check_suite.head_sha).await?;
    }

    Ok(())
}
