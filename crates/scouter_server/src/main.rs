pub mod alerts;
pub mod api;
pub mod consumer;
pub mod sql;

use crate::alerts::base::DriftExecutor;
use crate::api::metrics::metrics_app;
use crate::api::router::AppState;
use crate::api::setup::{create_db_pool, setup_logging};
use crate::sql::postgres::PostgresClient;
use anyhow::Context;
use api::router::create_router;
use std::sync::Arc;
use tracing::{error, info};

#[cfg(feature = "kafka")]
use crate::consumer::kafka::startup::kafka_startup::startup_kafka;

#[cfg(feature = "rabbitmq")]
use crate::consumer::rabbitmq::startup::rabbitmq_startup::startup_rabbitmq;

async fn start_metrics_server() -> Result<(), anyhow::Error> {
    let app = metrics_app().with_context(|| "Failed to setup metrics app")?;

    // NOTE: expose metrics endpoint on a different port
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8001")
        .await
        .with_context(|| "Failed to bind to port 8001 for metrics server")?;
    axum::serve(listener, app)
        .await
        .with_context(|| "Failed to start metrics server")?;

    Ok(())
}

async fn start_main_server() -> Result<(), anyhow::Error> {
    // setup logging
    setup_logging()
        .await
        .with_context(|| "Failed to setup logging")?;

    // db for app state and kafka
    let pool = create_db_pool(None)
        .await
        .with_context(|| "Failed to create Postgres client")?;

    // // run migrations
    sqlx::migrate!().run(&pool).await?;

    // setup background kafka task if kafka is enabled
    #[cfg(feature = "kafka")]
    if std::env::var("KAFKA_BROKERS").is_ok() {
        startup_kafka(pool.clone()).await?;
    }

    // setup background rabbitmq task if rabbitmq is enabled
    #[cfg(feature = "rabbitmq")]
    if std::env::var("RABBITMQ_ADDR").is_ok() {
        startup_rabbitmq(pool.clone()).await?;
    }

    // ##################### run drift polling background tasks #####################
    let num_scheduler_workers = std::env::var("SCOUTER_SCHEDULE_WORKER_COUNT")
        .unwrap_or_else(|_| "4".to_string())
        .parse::<usize>()
        .with_context(|| "Failed to parse SCOUTER_SCHEDULE_WORKER_COUNT")?;

    for i in 0..num_scheduler_workers {
        info!("Starting drift schedule poller: {}", i);
        let alert_db_client = PostgresClient::new(pool.clone())
            .with_context(|| "Failed to create Postgres client")?;
        tokio::task::spawn(async move {
            let mut drift_executor = DriftExecutor::new(alert_db_client);
            loop {
                if let Err(e) = drift_executor.poll_for_tasks().await {
                    error!("Alert poller error: {:?}", e);
                }
            }
        });
    }

    // start server
    let server_db_client =
        PostgresClient::new(pool.clone()).with_context(|| "Failed to create Postgres client")?;

    let app = create_router(Arc::new(AppState {
        db: server_db_client,
    }));

    let port = std::env::var("SCOUTER_SERVER_PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| "Failed to bind to port 8000")?;

    info!("🚀 Scouter Server started successfully");
    axum::serve(listener, app)
        .await
        .with_context(|| "Failed to start main server")?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (_main_server, _metrics_server) = tokio::join!(start_main_server(), start_metrics_server());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready`

    #[tokio::test]
    async fn test_health_check() {
        let pool = create_db_pool(Some(
            "postgresql://postgres:admin@localhost:5432/scouter?".to_string(),
        ))
        .await
        .with_context(|| "Failed to create Postgres client")
        .unwrap();

        let db_client = sql::postgres::PostgresClient::new(pool).unwrap();

        let app = create_router(Arc::new(AppState {
            db: db_client.clone(),
        }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/scouter/healthcheck")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();

        let v: Value = serde_json::from_str(std::str::from_utf8(&body[..]).unwrap()).unwrap();

        let message: &str = v.get("message").unwrap().as_str().unwrap();

        assert_eq!(message, "Alive");
    }
}
