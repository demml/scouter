use crate::api::routes::user::utils::get_user;
use crate::api::state::AppState;
use scouter_auth::permission::UserPermissions;
use scouter_sql::sql::traits::UserSqlLogic;
use scouter_sql::PostgresClient;
use std::sync::Arc;
use tonic::body::Body;
use tonic::codegen::http::{HeaderValue, Request};
use tonic::metadata::MetadataMap;
use tonic::{async_trait, Status};
use tonic_middleware::RequestInterceptor;
use tracing::{error, info, instrument};

const AUTHORIZATION: &str = "authorization";
const X_REFRESHED_TOKEN: &str = "x-refreshed-token";
const X_USERNAME: &str = "x-username";

#[derive(Clone)]
pub struct AuthInterceptor {
    state: Arc<AppState>,
}

impl AuthInterceptor {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl RequestInterceptor for AuthInterceptor {
    #[instrument(skip_all)]
    async fn intercept(&self, mut req: Request<Body>) -> Result<Request<Body>, Status> {
        // Extract bearer token from HTTP headers (not metadata)
        let token = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Status::unauthenticated("Missing authorization token"))?;

        // Validate token
        match self.state.auth_manager.validate_jwt(token) {
            Ok(claims) => {
                // Token is valid - set username in header for downstream services
                let username_header = HeaderValue::from_str(&claims.sub)
                    .map_err(|_| Status::internal("Failed to set username header"))?;

                req.headers_mut().insert(X_USERNAME, username_header);

                // Store permissions in request extensions
                let auth_middleware = UserPermissions {
                    username: claims.sub,
                    permissions: claims.permissions,
                    group_permissions: claims.group_permissions,
                };
                req.extensions_mut().insert(auth_middleware);

                Ok(req)
            }
            Err(_) => {
                info!("Access token expired, attempting refresh");

                // Decode without validation to get claims
                let expired_claims = self
                    .state
                    .auth_manager
                    .decode_jwt_without_validation(token)
                    .map_err(|_| Status::unauthenticated("Invalid token format"))?;

                // Get user from database
                let mut user = get_user(&self.state, &expired_claims.sub)
                    .await
                    .map_err(|_| {
                        error!("Failed to get user for token refresh");
                        Status::unauthenticated("User not found")
                    })?;

                // Validate stored refresh token
                if let Some(stored_refresh) = user.refresh_token.as_ref() {
                    if self
                        .state
                        .auth_manager
                        .validate_refresh_token(stored_refresh)
                        .is_ok()
                    {
                        // Generate new tokens
                        let new_access_token = self.state.auth_manager.generate_jwt(&user);
                        let new_refresh_token =
                            self.state.auth_manager.generate_refresh_token(&user);

                        // Update refresh token in database
                        user.refresh_token = Some(new_refresh_token);

                        PostgresClient::update_user(&self.state.db_pool, &user)
                            .await
                            .map_err(|_| {
                                error!("Failed to update refresh token in database");
                                Status::internal("Failed to update refresh token")
                            })?;

                        // Store permissions in request extensions
                        let auth_middleware = UserPermissions {
                            username: user.username.clone(),
                            permissions: user.permissions,
                            group_permissions: user.group_permissions,
                        };
                        req.extensions_mut().insert(auth_middleware);

                        // Set username header for downstream services
                        let username_header = HeaderValue::from_str(&user.username)
                            .map_err(|_| Status::internal("Failed to set username header"))?;
                        req.headers_mut().insert(X_USERNAME, username_header);

                        // Add new token to headers so it can be returned to client
                        let mut metadata = MetadataMap::new();
                        let token_value = format!("Bearer {}", new_access_token)
                            .parse()
                            .map_err(|_| Status::internal("Failed to parse token value"))?;
                        metadata.insert(X_REFRESHED_TOKEN, token_value);

                        info!("Token successfully refreshed");

                        // Store metadata in extensions for handler to access
                        req.extensions_mut().insert(metadata);

                        return Ok(req);
                    }
                }

                error!("Refresh token invalid or missing");
                Err(Status::unauthenticated("Token refresh failed"))
            }
        }
    }
}
