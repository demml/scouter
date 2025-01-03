use crate::api::middleware::track_metrics;
use crate::api::routes::{
    get_alert_router, get_drift_router, get_health_router, get_observability_router,
    get_profile_router,
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

/// Create the main router for the application
///
/// This function creates the main router for the application by merging all the sub-routers
/// and adding the necessary middleware.
///
/// # Parameters
/// - `app_state` - The application state shared across all handlers
///
/// # Returns
///
/// The main router for the application
pub async fn create_router(app_state: Arc<AppState>) -> Result<Router> {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::PUT, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    let health_routes = get_health_router(ROUTE_PREFIX).await?;
    let drift_routes = get_drift_router(ROUTE_PREFIX).await?;
    let profile_routes = get_profile_router(ROUTE_PREFIX).await?;
    let alert_routes = get_alert_router(ROUTE_PREFIX).await?;
    let observability_routes = get_observability_router(ROUTE_PREFIX).await?;

    let merged_routes = Router::new()
        .merge(health_routes)
        .merge(drift_routes)
        .merge(profile_routes)
        .merge(alert_routes)
        .merge(observability_routes)
        .route_layer(middleware::from_fn(track_metrics));

    Ok(Router::new()
        .merge(merged_routes)
        .layer(cors)
        .with_state(app_state))
}
