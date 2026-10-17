use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use code_reviewer_core::{
    self as core,
    analysis::{
        call_trace::GetCallTraceResponse,
        error::AnalysisError,
        function_source::FunctionSource,
        service::{
            AnalysisService, AnalyzeChangesRequest, AnalyzeChangesResponse,
            AnalyzeFileChangesResponse, GetCallTraceRequest, GetFunctionSourceRequest,
        },
    },
    git::{GitError, ListChangedFilesRequest, ListChangedFilesResponse},
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

type ApiError = (StatusCode, String);

fn analysis_error_response(error: AnalysisError) -> ApiError {
    let status = match error {
        AnalysisError::FunctionNotFound(_) => StatusCode::NOT_FOUND,
        AnalysisError::Git(GitError::InvalidRevision(_)) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, error.to_string())
}

/// Runs analysis on a blocking thread, since it reads files and runs git.
async fn run_analysis<T, F>(service: Arc<AnalysisService>, work: F) -> Result<Json<T>, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&AnalysisService) -> Result<T, AnalysisError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || work(&service))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(analysis_error_response)
}

async fn analyze_changes(
    State(service): State<Arc<AnalysisService>>,
    Json(request): Json<AnalyzeChangesRequest>,
) -> Result<Json<AnalyzeChangesResponse>, ApiError> {
    run_analysis(service, move |service| service.analyze_changes(request)).await
}

async fn analyze_file_changes(
    State(service): State<Arc<AnalysisService>>,
    Json(request): Json<AnalyzeChangesRequest>,
) -> Result<Json<AnalyzeFileChangesResponse>, ApiError> {
    run_analysis(service, move |service| {
        service.analyze_file_changes(request)
    })
    .await
}

async fn get_function_source(
    State(service): State<Arc<AnalysisService>>,
    Json(request): Json<GetFunctionSourceRequest>,
) -> Result<Json<FunctionSource>, ApiError> {
    run_analysis(service, move |service| service.get_function_source(request)).await
}

async fn get_call_trace(
    State(service): State<Arc<AnalysisService>>,
    Json(request): Json<GetCallTraceRequest>,
) -> Result<Json<GetCallTraceResponse>, ApiError> {
    run_analysis(service, move |service| service.get_call_trace(request)).await
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}

fn api_routes() -> Router {
    let analysis_service = Arc::new(AnalysisService::new());

    Router::new()
        .route("/api/health", get(health))
        .route("/api/greet", post(greet))
        .route("/api/list_changed_files", post(list_changed_files))
        .route("/api/analyze_changes", post(analyze_changes))
        .route("/api/analyze_file_changes", post(analyze_file_changes))
        .route("/api/get_function_source", post(get_function_source))
        .route("/api/get_call_trace", post(get_call_trace))
        .with_state(analysis_service)
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
