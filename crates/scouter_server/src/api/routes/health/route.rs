
use crate::api::state::AppState;
use crate::api::routes::Alive;
use axum::{routing::get, Router};
use anyhow::{Context, Result};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

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
