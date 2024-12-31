use crate::api::handler::{
    get_drift, get_drift_alerts, get_observability_metrics, get_profile, health_check,
    insert_drift, insert_drift_profile, update_drift_profile_status,
};
use crate::api::metrics::track_metrics;
use crate::sql::postgres::PostgresClient;
use axum::http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Method,
};
use axum::middleware;
use axum::{
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use super::handler::update_drift_profile;

const ROUTE_PREFIX: &str = "/scouter";

pub struct AppState {
    pub db: PostgresClient,
}

pub fn create_router(app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::PUT, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    Router::new()
        .route(&format!("{}/healthcheck", ROUTE_PREFIX), get(health_check))
        .route(
            &format!("{}/drift", ROUTE_PREFIX),
            get(get_drift).post(insert_drift),
        )
        .route(
            &format!("{}/profile", ROUTE_PREFIX),
            post(insert_drift_profile)
                .put(update_drift_profile)
                .get(get_profile),
        )
        .route(
            &format!("{}/profile/status", ROUTE_PREFIX),
            put(update_drift_profile_status),
        )
        .route(&format!("{}/alerts", ROUTE_PREFIX), get(get_drift_alerts))
        .route(
            &format!("{}/observability/metrics", ROUTE_PREFIX),
            get(get_observability_metrics),
        )
        .route_layer(middleware::from_fn(track_metrics))
        .with_state(app_state)
        .layer(cors)
}
