use crate::api::state::AppState;
use anyhow::Result;
/// Route for debugging information
use axum::{http::StatusCode, Json};
use scouter_sql::sql::schema::User;
use std::sync::Arc;
use tracing::error;

/// Resuable function to get a user from the database
///
/// # Parameters
///
/// - `state` - The application state
/// - `username` - The username of the user to get
///
/// # Returns
///
/// Returns a `Result` containing either the user or an error
///
/// # Errors
///
/// Returns an error if the user is not found in the database
///
/// # Panics
///
/// Panics if the user cannot be retrieved from the database
pub async fn get_user(
    state: &Arc<AppState>,
    username: &str,
) -> Result<User, (StatusCode, Json<serde_json::Value>)> {
    state
        .db
        .get_user(username)
        .await
        .map_err(|e| {
            error!("Failed to get user from database: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({})),
            )
        })?
        .ok_or_else(|| {
            error!("User not found in database");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "User not found" })),
            )
        })
}
