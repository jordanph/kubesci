use log::error;
use std::net::SocketAddr;
use warp::Filter;

use k8s_openapi::api::core::v1::Pod;
use kube::{api::Api, Client};

#[macro_use]
extern crate vec1;

use handlers::{
    check_run::handle_check_run_request, check_suite::handle_check_suite_request,
    pipeline::handle_get_pipeline, pipelines::handle_get_pipelines, steps::handle_get_steps,
};
use pipeline::PipelineService;
use routes::{
    check_run_route, check_suite_route, get_pipeline_route, get_pipeline_steps_route,
    get_pipelines_route,
};

use pod_informer::PodInformer;

mod config;
mod github;
mod handlers;
mod kubernetes;
mod pipeline;
mod pod_informer;
mod routes;

#[tokio::main]
async fn main() {
    let _ = pretty_env_logger::try_init();

    match config::Config::new() {
        Ok(config) => {
            let pipeline_service = PipelineService {
                github_private_key: config.github_private_key.clone(),
                application_id: config.application_id.clone(),
                namespace: config.namespace.clone(),
                github_base_url: config.github_base_url.clone(),
            };

            let pipeline_service_handler = warp::any().map(move || pipeline_service.clone());

            let check_suite_handler = check_suite_route()
                .and(pipeline_service_handler.clone())
                .and_then(handle_check_suite_request);

            let check_run_handler = check_run_route()
                .and(pipeline_service_handler.clone())
                .and_then(handle_check_run_request);

            let cors = warp::cors().allow_origin("http://localhost:3000");

            let get_pipelines_handler = get_pipelines_route()
                .and_then(handle_get_pipelines)
                .with(cors);

            let pipeline_cors = warp::cors().allow_origin("http://localhost:3000");

            let get_pipeline_handler = get_pipeline_route()
                .and_then(handle_get_pipeline)
                .with(pipeline_cors);

            let steps_cors = warp::cors().allow_origin("http://localhost:3000");

            let get_pipeline_steps_handler = get_pipeline_steps_route()
                .and_then(handle_get_steps)
                .with(steps_cors);

            let app_routes = check_suite_handler
                .or(check_run_handler)
                .or(get_pipeline_steps_handler)
                .or(get_pipeline_handler)
                .or(get_pipelines_handler);

            let address =
                std::env::var("SOCKET_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = std::env::var("PORT").unwrap_or_else(|_| "3030".to_string());

            let socket_address: SocketAddr = format!("{}:{}", address, port)
                .parse()
                .expect("Unable to parse socket address");

            let client = Client::try_default().await.unwrap();
            let pods: Api<Pod> = Api::namespaced(client, "kubesci");

            let pipeline_service_pod = PipelineService {
                github_private_key: config.github_private_key.clone(),
                application_id: config.application_id.clone(),
                namespace: config.namespace.clone(),
                github_base_url: config.github_base_url.clone(),
            };

            let pod_informer = PodInformer {
                pipeline_service: pipeline_service_pod,
                pods_api: pods,
                github_private_key: config.github_private_key.clone(),
                application_id: config.application_id.clone(),
                github_base_url: config.github_base_url.clone(),
            };

            let tasks = vec![
                tokio::spawn(async move { warp::serve(app_routes).run(socket_address).await }),
                tokio::spawn(async move { pod_informer.poll_pods().await }),
            ];

            futures::future::join_all(tasks).await;
        }
        Err(var_error) => error!("Config error: {}", var_error),
    }
}
