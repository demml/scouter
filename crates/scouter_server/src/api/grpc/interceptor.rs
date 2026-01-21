use crate::api::routes::user::utils::get_user;
use crate::api::state::AppState;
use jsonwebtoken::errors::ErrorKind;
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
    pub(crate) fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
    async fn handle_token_refresh(
        &self,
        token: &str,
    ) -> Result<(UserPermissions, Option<String>), Status> {
        info!("Access token expired, attempting refresh");

        let expired_claims = self
            .state
            .auth_manager
            .decode_jwt_without_validation(token)
            .map_err(|_| Status::unauthenticated("Invalid token format"))?;

        let mut user = get_user(&self.state, &expired_claims.sub)
            .await
            .map_err(|_| {
                error!("Failed to get user for token refresh");
                Status::unauthenticated("User not found")
            })?;

        let stored_refresh = user.refresh_token.as_ref().ok_or_else(|| {
            error!("No refresh token found for user: {}", user.username);
            Status::unauthenticated("Refresh token not found")
        })?;

        self.state
            .auth_manager
            .validate_refresh_token(stored_refresh)
            .map_err(|_| {
                error!("Invalid refresh token for user: {}", user.username);
                Status::unauthenticated("Invalid refresh token")
            })?;

        let new_access_token = self.state.auth_manager.generate_jwt(&user);
        let new_refresh_token = self.state.auth_manager.generate_refresh_token(&user);

        user.refresh_token = Some(new_refresh_token);

        PostgresClient::update_user(&self.state.db_pool, &user)
            .await
            .map_err(|e| {
                error!("Failed to update refresh token in database: {}", e);
                Status::internal("Failed to update refresh token")
            })?;

        let auth_middleware = UserPermissions {
            username: user.username.clone(),
            permissions: user.permissions,
            group_permissions: user.group_permissions,
        };

        info!("Token successfully refreshed for user: {}", user.username);

        Ok((auth_middleware, Some(new_access_token)))
    }

    fn set_response_headers(
        &self,
        req: &mut Request<Body>,
        username: &str,
        refreshed_token: Option<String>,
    ) -> Result<(), Status> {
        let username_header = HeaderValue::from_str(username)
            .map_err(|_| Status::internal("Failed to set username header"))?;
        req.headers_mut().insert(X_USERNAME, username_header);

        if let Some(token) = refreshed_token {
            let mut metadata = MetadataMap::new();
            let token_value = format!("Bearer {}", token)
                .parse()
                .map_err(|_| Status::internal("Failed to parse token value"))?;
            metadata.insert(X_REFRESHED_TOKEN, token_value);
            req.extensions_mut().insert(metadata);
        }

        Ok(())
    }
}

#[async_trait]
impl RequestInterceptor for AuthInterceptor {
    #[instrument(skip_all)]
    async fn intercept(&self, mut req: Request<Body>) -> Result<Request<Body>, Status> {
        let token = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("Missing authorization token"))?;

        match self.state.auth_manager.validate_jwt(token) {
            Ok(claims) => {
                let auth_middleware = UserPermissions {
                    username: claims.sub.clone(),
                    permissions: claims.permissions,
                    group_permissions: claims.group_permissions,
                };
                req.extensions_mut().insert(auth_middleware);
                self.set_response_headers(&mut req, &claims.sub, None)?;
                Ok(req)
            }
            Err(e) => match e.kind() {
                ErrorKind::ExpiredSignature => {
                    let (auth_middleware, refreshed_token) =
                        self.handle_token_refresh(token).await?;
                    let username = auth_middleware.username.clone();
                    req.extensions_mut().insert(auth_middleware);
                    self.set_response_headers(&mut req, &username, refreshed_token)?;
                    Ok(req)
                }
                _ => {
                    error!("Token validation failed: {:?}", e.kind());
                    Err(Status::unauthenticated("Invalid token"))
                }
            },
        }
    }
}
