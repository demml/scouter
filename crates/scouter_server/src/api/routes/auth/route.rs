use crate::api::routes::auth::schema::Authenticated;
use crate::api::routes::user::utils::get_user;
use crate::api::state::AppState;
use anyhow::{Context, Result};
/// Route for debugging information
use axum::extract::State;
use axum::{http::header, http::header::HeaderMap, http::StatusCode, routing::get, Json, Router};

use scouter_contracts::ScouterServerError;
use scouter_types::JwtToken;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{error, instrument};

/// Route for the login endpoint when using the API
///
/// # Parameters
///
/// - `state` - The application state
/// - `headers` - The headers from the request
///
/// # Returns
///
/// Returns a `Result` containing either the JWT token or an error
#[instrument(skip_all)]
pub async fn api_login_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<JwtToken>, (StatusCode, Json<ScouterServerError>)> {
    // get Username and Password from headers
    let username = headers
        .get("Username")
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::username_header_not_found()),
            )
        })?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::invalid_username_format()),
            )
        })?
        .to_string();

    let password = headers
        .get("Password")
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::password_header_not_found()),
            )
        })?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::invalid_password_format()),
            )
        })?
        .to_string();

    // get user from database
    let mut user = get_user(&state, &username).await?;

    // check if password is correct
    state
        .auth_manager
        .validate_user(&user, &password)
        .map_err(|e| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ScouterServerError::unauthorized(e)),
            )
        })?;

    // we may get multiple requests for the same user (setting up storage and registries), so we
    // need to check if current refresh and jwt tokens are valid and return them if they are

    // generate JWT token
    let jwt_token = state.auth_manager.generate_jwt(&user);

    // check if refresh token is already set.
    // if it is, check if its valid and return it
    // if it is not, generate a new one
    if let Some(refresh_token) = &user.refresh_token {
        if state
            .auth_manager
            .validate_refresh_token(refresh_token)
            .is_ok()
        {
            return Ok(Json(JwtToken { token: jwt_token }));
        }
    }

    let refresh_token = state.auth_manager.generate_refresh_token(&user);
    user.refresh_token = Some(refresh_token);

    // set refresh token in db
    state.db.update_user(&user).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::refresh_token_error(e)),
        )
    })?;

    Ok(Json(JwtToken { token: jwt_token }))
}

/// Route for the refresh token endpoint when using the API
///
/// # Parameters
///
/// - `state` - The application state
/// - `headers` - The headers from the request
///
/// # Returns
///
/// Returns a `Result` containing either the JWT token or an error
pub async fn api_refresh_token_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<JwtToken>, (StatusCode, Json<ScouterServerError>)> {
    let bearer_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_value| {
            auth_value
                .strip_prefix("Bearer ")
                .map(|token| token.to_owned())
        });

    if let Some(bearer_token) = bearer_token {
        // validate the refresh token
        let claims = state
            .auth_manager
            .decode_jwt_without_validation(&bearer_token)
            .map_err(|e| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ScouterServerError::jwt_decode_error(e.to_string())),
                )
            })?;

        // get user from database
        let mut user = get_user(&state, &claims.sub).await?;

        // generate JWT token
        let jwt_token = state.auth_manager.generate_jwt(&user);

        // generate refresh token
        let refresh_token = state.auth_manager.generate_refresh_token(&user);

        user.refresh_token = Some(refresh_token);

        // set refresh token in db
        state.db.update_user(&user).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::refresh_token_error(e)),
            )
        })?;

        Ok(Json(JwtToken { token: jwt_token }))
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(ScouterServerError::no_refresh_token()),
        ))
    }
}

async fn validate_jwt_token(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Authenticated>, (StatusCode, Json<ScouterServerError>)> {
    let bearer_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_value| {
            auth_value
                .strip_prefix("Bearer ")
                .map(|token| token.to_owned())
        });

    if let Some(bearer_token) = bearer_token {
        match state.auth_manager.validate_jwt(&bearer_token) {
            Ok(_) => Ok(Json(Authenticated {
                is_authenticated: true,
            })),
            Err(_) => Err((
                StatusCode::UNAUTHORIZED,
                Json(ScouterServerError::failed_token_validation()),
            )),
        }
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(ScouterServerError::bearer_token_not_found()),
        ))
    }
}

pub async fn get_auth_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{}/auth/login", prefix), get(api_login_handler))
            .route(
                &format!("{}/auth/refresh", prefix),
                get(api_refresh_token_handler),
            )
            .route(
                &format!("{}/auth/validate", prefix),
                get(validate_jwt_token),
            )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            error!("Failed to create auth router");
            // panic
            Err(anyhow::anyhow!("Failed to create auth router"))
                .context("Panic occurred while creating the router")
        }
    }
}
