use serde_derive::{Deserialize};
use std::convert::Infallible;
use warp::Filter;
use warp::http::StatusCode;
use log::{info};
use k8s_openapi::api::core::v1::Pod;
use serde_json::json;
use std::{thread, time};

use github::client::auth::GithubAuthorisationClient;
use github::client::installation::GithubInstallationClient;
use github::auth::authenticate_app;
use std::fs;

mod github;

use kube::{
    api::{Api, Meta, PostParams},
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

#[derive(Debug, Deserialize)]
pub struct Step {
    name: String,
    image: String,
    commands: std::vec::Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Steps {
    steps: std::vec::Vec<Step>,
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

    // Manage pods
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    
    info!("Creating Pod instance blog...");

    // TO DO: Download the steps from the actual repo...
    let contents = fs::read_to_string("/Users/jordan.holland/personal/kubes-cd/src/test.yaml")?;

    let yaml_steps: Steps = serde_yaml::from_str(&contents)?;

    info!("Decoded the yaml {:?}", yaml_steps);

    let containers: serde_json::value::Value = yaml_steps.steps.iter().map(|step| {
        return json!({
            "name": step.name.replace(" ", "-").to_lowercase(),
            "image": step.image,
            "command": step.commands
        });
    }).collect();

    info!("Containers to deploy {}", containers);

    let pod_deployment_config: Pod = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": github_webhook_request.check_run.check_suite.head_sha },
        "spec": {
            "containers": containers,
            "restartPolicy": "Never",
        }
    }))?;

    let pp = PostParams::default();
    match pods.create(&pp, &pod_deployment_config).await {
        Ok(o) => {
            let name = Meta::name(&o);
            info!("Created pod: {}!", name);
        }
        Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
        Err(e) => return Err(e.into()),                        // any other case is probably bad
    }

    info!("Pod has spun up! Notify Github that test is in progress...");

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

    github_installation_client.set_check_run_in_progress(github_webhook_request.check_run.id).await?;

    info!("Waiting for pod to finish to return completion state...");

    let termination_status = loop {
        let p1cpy = pods.get(&github_webhook_request.check_run.check_suite.head_sha).await?;
        if let Some(status) = p1cpy.status {
            if let Some(container_statuses) = status.container_statuses {
                let maybe_container_status = container_statuses
                    .into_iter()
                    .find(|container_status| container_status.name == yaml_steps.steps.first().map_or("test".to_string(), |step| step.name.replace(" ", "-").to_lowercase()));

                if let Some(container_status) = maybe_container_status {
                    if let Some(state) = container_status.state {
                        if let Some(_) = state.running {
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

        thread::sleep(time::Duration::from_secs(5));
    };

    if termination_status.exit_code == 0 {
        github_installation_client.set_check_run_complete(github_webhook_request.check_run.started_at, github_webhook_request.check_run.id, "success".to_string()).await?;
    } else {
        github_installation_client.set_check_run_complete(github_webhook_request.check_run.started_at, github_webhook_request.check_run.id, "failure".to_string()).await?;
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

    // TO DO: Create a check run per step in the CI pipeline (only create 1 if file is invalid)
    github_installation_client.create_check_run(github_webhook_request.check_suite.head_sha).await?;

    Ok(())
}
