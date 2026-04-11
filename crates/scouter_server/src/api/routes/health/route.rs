use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::response::IntoResponse;
use axum::Json;
use axum::{routing::get, Router};
/// file containing schema for health module
use serde::{Deserialize, Serialize};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct Alive {
    pub alive: bool,
    pub version: String,
}

impl Default for Alive {
    fn default() -> Self {
        Self {
            alive: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

// Implement IntoResponse for Alive
impl IntoResponse for Alive {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

#[utoipa::path(
    get,
    path = "/scouter/healthcheck",
    responses(
        (status = 200, description = "Server is healthy", body = Alive),
    ),
    tag = "health"
)]
pub async fn health_check() -> Alive {
    Alive::default()
}

pub async fn get_health_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(&format!("{prefix}/healthcheck"), get(health_check))
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create health router"))
                .context("Panic occurred while creating the router")
        }
    }
}
