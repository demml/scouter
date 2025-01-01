
use crate::api::middleware::track_metrics;
use crate::api::state::AppState;
use crate::api::routes::{get_health_router, get_drift_router, get_profile_router};
use anyhow::Result;
use axum::http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Method,
};
use axum::middleware;
use axum::Router;
use scouter_types::custom::profile;
use std::sync::Arc;
use tower_http::cors::CorsLayer;


const ROUTE_PREFIX: &str = "/scouter";

pub async fn create_router(app_state: Arc<AppState>) -> Result<Router> {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::PUT, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

        let health_routes = get_health_router(ROUTE_PREFIX).await?;
        let drift_routes = get_drift_router(ROUTE_PREFIX).await?;
        let profile_routes = get_profile_router(ROUTE_PREFIX).await?;

        let merged_routes = Router::new()
        .merge(health_routes)
        .merge(drift_routes)
        .merge(profile_routes)
        .route_layer(middleware::from_fn(track_metrics));

        Ok(Router::new()
        .merge(merged_routes)
        .layer(cors)
        .with_state(app_state))
    // let router = Router::new()
    //outer::new()
    //   .route(&format!("{}/healthcheck", ROUTE_PREFIX), get(health_check))
    //   .route(
    //       &format!("{}/drift", ROUTE_PREFIX),
    //       get(get_drift).post(insert_drift),
    //   )
    //   .route(
    //       &format!("{}/profile", ROUTE_PREFIX),
    //       post(insert_drift_profile)
    //           .put(update_drift_profile)
    //           .get(get_profile),
    //   )
    //   .route(
    //       &format!("{}/profile/status", ROUTE_PREFIX),
    //       put(update_drift_profile_status),
    //   )
    //   .route(&format!("{}/alerts", ROUTE_PREFIX), get(get_drift_alerts))
    //   .route(
    //       &format!("{}/observability/metrics", ROUTE_PREFIX),
    //       get(get_observability_metrics),
    //   )
    //   .route_layer(middleware::from_fn(track_metrics))
    //   .with_state(app_state)
    //   .layer(cors)
}
