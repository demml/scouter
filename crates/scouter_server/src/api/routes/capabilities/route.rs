use axum::{routing::get, Json, Router};
use crate::api::state::AppState;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize, utoipa::ToSchema)]
pub struct CapabilitiesResponse {
    #[schema(value_type = String)]
    pub api_version: &'static str,
    #[schema(value_type = String)]
    pub server_version: &'static str,
    pub features: FeaturesInfo,
    pub endpoints: EndpointsInfo,
    pub auth: AuthInfo,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct FeaturesInfo {
    pub drift_detection: bool,
    pub distributed_tracing: bool,
    pub agent_evaluation: bool,
    pub alerting: bool,
    pub datasets: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct EndpointsInfo {
    #[schema(value_type = String)]
    pub drift_spc: &'static str,
    #[schema(value_type = String)]
    pub drift_psi: &'static str,
    #[schema(value_type = String)]
    pub drift_custom: &'static str,
    #[schema(value_type = String)]
    pub profile_insert: &'static str,
    #[schema(value_type = String)]
    pub profile_list: &'static str,
    #[schema(value_type = String)]
    pub alerts_list: &'static str,
    #[schema(value_type = String)]
    pub traces_baggage: &'static str,
    #[schema(value_type = String)]
    pub traces_paginated: &'static str,
    #[schema(value_type = String)]
    pub agent_records: &'static str,
    #[schema(value_type = String)]
    pub datasets_list: &'static str,
    #[schema(value_type = String)]
    pub tags_list: &'static str,
    #[schema(value_type = String)]
    pub observability_metrics: &'static str,
    #[schema(value_type = String)]
    pub openapi_spec: &'static str,
    #[schema(value_type = String)]
    pub swagger_ui: &'static str,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct AuthInfo {
    #[schema(value_type = String)]
    pub auth_type: &'static str,
    #[schema(value_type = String)]
    pub login_endpoint: &'static str,
    #[schema(value_type = String)]
    pub refresh_endpoint: &'static str,
    #[schema(value_type = String)]
    pub token_header: &'static str,
}

#[utoipa::path(
    get,
    path = "/scouter/api/v1/capabilities",
    responses(
        (status = 200, description = "Server capabilities and endpoint index", body = CapabilitiesResponse),
    ),
    tag = "capabilities"
)]
pub async fn capabilities() -> Json<CapabilitiesResponse> {
    Json(CapabilitiesResponse {
        api_version: "1.0.0",
        server_version: env!("CARGO_PKG_VERSION"),
        features: FeaturesInfo {
            drift_detection: true,
            distributed_tracing: true,
            agent_evaluation: true,
            alerting: true,
            datasets: true,
        },
        endpoints: EndpointsInfo {
            drift_spc: "GET /scouter/drift/spc",
            drift_psi: "GET /scouter/drift/psi",
            drift_custom: "GET /scouter/drift/custom",
            profile_insert: "POST /scouter/profile",
            profile_list: "POST /scouter/profiles",
            alerts_list: "POST /scouter/alerts",
            traces_baggage: "GET /scouter/trace/baggage",
            traces_paginated: "POST /scouter/trace/paginated",
            agent_records: "POST /scouter/agent/page/record",
            datasets_list: "GET /scouter/datasets",
            tags_list: "GET /scouter/tags",
            observability_metrics: "GET /scouter/observability/metrics",
            openapi_spec: "/scouter/api/v1/openapi.json",
            swagger_ui: "/scouter/api/v1/docs/ui",
        },
        auth: AuthInfo {
            auth_type: "bearer",
            login_endpoint: "GET /scouter/auth/login",
            refresh_endpoint: "GET /scouter/auth/refresh",
            token_header: "Authorization: Bearer <token>",
        },
    })
}

pub fn get_capabilities_router(prefix: &str) -> Router<Arc<AppState>> {
    Router::new().route(
        &format!("{prefix}/api/v1/capabilities"),
        get(capabilities),
    )
}
