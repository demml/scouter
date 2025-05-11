use crate::api::routes::user::schema::{
    CreateUserRequest, UpdateUserRequest, UserListResponse, UserResponse,
};
use crate::api::routes::user::utils::get_user as get_user_from_db;
use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::extract::Path;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use password_auth::generate_hash;
use scouter_auth::permission::UserPermissions;
use scouter_sql::sql::schema::User;
use scouter_sql::sql::traits::UserSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::contracts::{ScouterResponse, ScouterServerError};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{error, info, instrument};

pub async fn initialize_users(
    State(state): State<AppState>,
    Json(users): Json<Vec<User>>,
) -> Result<(), StatusCode> {
    // Only allow initialization if no users exist
    if !PostgresClient::get_users(&state.db_pool)
        .await
        .map_err(|e| {
            error!("Failed to check existing users: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .is_empty()
    {
        return Err(StatusCode::FORBIDDEN);
    }

    for user in users {
        PostgresClient::insert_user(&state.db_pool, &user)
            .await
            .map_err(|e| {
                error!("Failed to insert user: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(())
}

/// Create a new user via SDK
///
/// Requires admin permissions
async fn create_user(
    State(state): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(create_req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Check if requester has admin permissions
    if !perms.group_permissions.contains(&"admin".to_string()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::need_admin_permission()),
        ));
    }

    // Check if user already exists
    if let Ok(Some(_)) = PostgresClient::get_user(&state.db_pool, &create_req.username).await {
        return Err((
            StatusCode::CONFLICT,
            Json(ScouterServerError::user_already_exists()),
        ));
    }

    // Hash the password
    let password_hash = generate_hash(&create_req.password);

    // Create the user
    let mut user = User::new(
        create_req.username,
        password_hash,
        create_req.permissions,
        create_req.group_permissions,
        create_req.role,
    );

    // Set active status if provided
    if let Some(active) = create_req.active {
        user.active = active;
    }

    // Save to database
    if let Err(e) = PostgresClient::insert_user(&state.db_pool, &user).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::create_user_error(e)),
        ));
    }

    info!("User {} created successfully", user.username);
    Ok(Json(UserResponse::from(user)))
}

/// Get a user by username
#[instrument(skip_all)]
async fn get_user(
    State(state): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Path(username): Path<String>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Check permissions - user can only get their own data or admin can get any user
    let is_admin = perms.group_permissions.contains(&"admin".to_string());
    let is_self = perms.username == username;

    if !is_admin && !is_self {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    // Get user from database
    let user = match PostgresClient::get_user(&state.db_pool, &username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ScouterServerError::user_not_found()),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_user_error(e)),
            ));
        }
    };

    Ok(Json(UserResponse::from(user)))
}

/// List all users
///
/// Requires admin permissions
#[instrument(skip_all)]
async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<UserListResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Check if requester has admin permissions
    if !perms.group_permissions.contains(&"admin".to_string()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::need_admin_permission()),
        ));
    }

    // Get users from database
    let users = match PostgresClient::get_users(&state.db_pool).await {
        Ok(users) => users,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::list_users_error(e)),
            ));
        }
    };

    let user_responses: Vec<UserResponse> = users.into_iter().map(UserResponse::from).collect();

    Ok(Json(UserListResponse {
        users: user_responses,
    }))
}

/// Update a user
#[instrument(skip_all)]
async fn update_user(
    State(state): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Path(username): Path<String>,
    Json(update_req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Check permissions - user can only update their own data or admin can update any user
    let is_admin = perms.group_permissions.contains(&"admin".to_string());
    let is_self = perms.username == username;

    if !is_admin && !is_self {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    // Get the current user state
    let mut user = match get_user_from_db(&state, &username).await {
        Ok(user) => user,
        Err((status, error)) => {
            return Err((status, error));
        }
    };

    // Update fields based on request
    if let Some(password) = update_req.password {
        user.password_hash = generate_hash(&password);
    }

    // Only admins can change permissions
    if is_admin {
        if let Some(permissions) = update_req.permissions {
            user.permissions = permissions;
        }

        if let Some(group_permissions) = update_req.group_permissions {
            user.group_permissions = group_permissions;
        }

        if let Some(active) = update_req.active {
            user.active = active;
        }
    }

    // Save updated user to database
    if let Err(e) = PostgresClient::update_user(&state.db_pool, &user).await {
        error!("Failed to update user: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::update_user_error(e)),
        ));
    }

    info!("User {} updated successfully", user.username);
    Ok(Json(UserResponse::from(user)))
}

/// Delete a user
///
/// Requires admin permissions
#[instrument(skip_all)]
async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Path(username): Path<String>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Check if requester has admin permissions
    if !perms.group_permissions.contains(&"admin".to_string()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::need_admin_permission()),
        ));
    }

    // Prevent deleting the last admin user
    let is_last_admin = match PostgresClient::is_last_admin(&state.db_pool, &username).await {
        Ok(is_last) => is_last,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::check_last_admin_error(e)),
            ));
        }
    };

    if is_last_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::cannot_delete_last_admin()),
        ));
    }

    // Delete the user
    if let Err(e) = PostgresClient::delete_user(&state.db_pool, &username).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::delete_user_error(e)),
        ));
    }

    info!("User {} deleted successfully", username);
    Ok(Json(ScouterResponse {
        status: "success".to_string(),
        message: format!("User {} deleted successfully", username),
    }))
}

pub async fn get_user_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{}/user", prefix), post(create_user))
            .route(&format!("{}/user", prefix), get(list_users))
            .route(&format!("{}/user/{{username}}", prefix), get(get_user))
            .route(&format!("{}/user/{{username}}", prefix), put(update_user))
            .route(
                &format!("{}/user/{{username}}", prefix),
                delete(delete_user),
            )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            error!("Failed to create user router");
            Err(anyhow::anyhow!("Failed to create user router"))
                .context("Panic occurred while creating the router")
        }
    }
}
