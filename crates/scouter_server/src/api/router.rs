use crate::api::middleware::track_metrics;
use crate::api::openapi::openapi_router;
use crate::api::routes::auth::auth_api_middleware;
use crate::api::routes::{
    get_agent_router, get_alert_router, get_auth_router, get_capabilities_router,
    get_dataset_router, get_docs_router, get_drift_router, get_eval_scenario_router,
    get_health_router, get_message_router, get_observability_router, get_profile_router,
    get_service_map_router, get_tag_router, get_trace_router, get_user_router,
};
use crate::api::state::AppState;
use anyhow::Result;
use axum::http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Method,
};
use axum::middleware;
use axum::Router;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

const ROUTE_PREFIX: &str = "/scouter";

async fn add_api_version_header(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-scouter-api-version",
        axum::http::HeaderValue::from_static("1.0.0"),
    );
    response
}

/// Create the main router for the application
pub async fn create_router(app_state: Arc<AppState>) -> Result<Router> {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    let health_routes = get_health_router(ROUTE_PREFIX).await?;
    let drift_routes = get_drift_router(ROUTE_PREFIX).await?;
    let profile_routes = get_profile_router(ROUTE_PREFIX).await?;
    let alert_routes = get_alert_router(ROUTE_PREFIX).await?;
    let auth_routes = get_auth_router(ROUTE_PREFIX).await?;
    let user_routes = get_user_router(ROUTE_PREFIX).await?;
    let tag_routes = get_tag_router(ROUTE_PREFIX).await?;
    let trace_routes = get_trace_router(ROUTE_PREFIX).await?;
    let message_routes = get_message_router(ROUTE_PREFIX).await?;
    let observability_routes = get_observability_router(ROUTE_PREFIX).await?;
    let agent_routes = get_agent_router(ROUTE_PREFIX).await?;
    let dataset_routes = get_dataset_router(ROUTE_PREFIX);
    let service_map_routes = get_service_map_router(ROUTE_PREFIX);
    let eval_scenario_routes = get_eval_scenario_router(ROUTE_PREFIX);
    let capabilities_routes = get_capabilities_router(ROUTE_PREFIX);
    let docs_routes = get_docs_router(ROUTE_PREFIX);
    let openapi_routes = tokio::task::spawn_blocking(openapi_router)
        .await
        .expect("Failed to build OpenAPI router");

    let merged_routes = Router::new()
        .merge(drift_routes)
        .merge(profile_routes)
        .merge(alert_routes)
        .merge(observability_routes)
        .merge(user_routes)
        .merge(tag_routes)
        .merge(trace_routes)
        .merge(agent_routes)
        .merge(message_routes)
        .merge(dataset_routes)
        .merge(service_map_routes)
        .merge(eval_scenario_routes)
        .route_layer(middleware::from_fn(track_metrics))
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_api_middleware,
        ));

    Ok(Router::new()
        .merge(merged_routes)
        .merge(health_routes)
        .merge(auth_routes)
        .merge(capabilities_routes)
        .merge(docs_routes)
        .merge(openapi_routes)
        .layer(middleware::from_fn(add_api_version_header))
        .layer(cors)
        .with_state(app_state))
}
