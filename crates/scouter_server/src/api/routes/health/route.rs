use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::response::IntoResponse;
use axum::Json;
use axum::{routing::get, Router};
use metrics::counter;
/// file containing schema for health module
use serde::{Deserialize, Serialize};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct Alive {
    pub status: String,
}

impl Default for Alive {
    fn default() -> Self {
        Self {
            status: "Alive".to_string(),
        }
    }
}

// Implement IntoResponse for Alive
impl IntoResponse for Alive {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

pub async fn health_check() -> Alive {
    Alive::default()
}

pub async fn get_health_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(&format!("{}/healthcheck", prefix), get(health_check))
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
