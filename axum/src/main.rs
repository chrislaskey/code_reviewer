use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use code_reviewer_core::{
    self as core,
    git::{ListChangedFilesRequest, ListChangedFilesResponse},
    GreetRequest, GreetResponse,
};
use tower_http::{cors::CorsLayer, services::ServeDir};

async fn greet(Json(request): Json<GreetRequest>) -> Json<GreetResponse> {
    Json(core::greet(request))
}

async fn list_changed_files(
    Json(request): Json<ListChangedFilesRequest>,
) -> Result<Json<ListChangedFilesResponse>, (StatusCode, String)> {
    core::git::list_changed_files(request)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}

fn api_routes() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/greet", post(greet))
        .route("/api/list_changed_files", post(list_changed_files))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let static_files = ServeDir::new("web/dist");

    let app = api_routes()
        .fallback_service(static_files)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Axum server listening on http://localhost:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
