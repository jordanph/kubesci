use serde_derive::Serialize;
use std::convert::Infallible;
use warp::Filter;
use warp::http::StatusCode;
use log::error;
use std::net::SocketAddr;

#[macro_use]
extern crate vec1;

use handlers::{get_pipelines, get_pipeline, get_pipeline_steps};
use handlers::check_suite::handle_check_suite_request;
use handlers::check_run::handle_update_check_run_request;
use routes::{check_suite_route, update_check_run_route};

mod github;
mod pipeline;
mod handlers;
mod routes;

#[tokio::main]
async fn main() {
    let _ = pretty_env_logger::try_init();

    let cors = warp::cors()
        .allow_origin("http://localhost:3000");

    let check_suite_handler = check_suite_route().and_then(handle_check_suite_request);

    let update_check_run_handler = update_check_run_route()
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
