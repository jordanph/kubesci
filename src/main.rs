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
mod github;
mod app_routes;

use kube::{
    api::{Api, Meta, PostParams},
    Client,
};

#[tokio::main]
async fn main() {
    let _ = pretty_env_logger::try_init();

    let app_routes = app_routes::app_routes();

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

    // Manage pods
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    
    info!("Creating Pod instance blog...");
    
    let pod_deployment_config: Pod = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "blog" },
        "spec": {
            "containers": [{
              "name": "myapp-container-1",
              "image": "busybox",
              "command": ["sh", "-c", "echo Hello Kubernetes! && sleep 20"]
            },
            ],
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
        let p1cpy = pods.get("blog").await?;
        if let Some(status) = p1cpy.status {
            if let Some(container_statuses) = status.container_statuses {
                let maybe_container_status = container_statuses
                    .into_iter()
                    .find(|container_status| container_status.name == "myapp-container-1");

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
        github_installation_client.set_check_run_complete(github_webhook_request.check_run.id, "success".to_string()).await?;
    } else {
        github_installation_client.set_check_run_complete(github_webhook_request.check_run.id, "failure".to_string()).await?;
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

    github_installation_client.create_check_run(github_webhook_request.check_suite.head_sha).await?;

    Ok(())
}
