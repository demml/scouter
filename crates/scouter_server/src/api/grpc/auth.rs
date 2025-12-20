use crate::api::routes::user::utils::get_user;
use crate::api::state::AppState;
use scouter_sql::sql::traits::UserSqlLogic;
use scouter_sql::PostgresClient;
use scouter_tonic::{
    AuthServiceServer, LoginRequest, LoginResponse, RefreshTokenRequest, RefreshTokenResponse,
    ValidateTokenRequest, ValidateTokenResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{debug, error, instrument};

pub struct AuthServiceImpl {
    state: Arc<AppState>,
}

impl AuthServiceImpl {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn into_service(self) -> AuthServiceServer<Self> {
        AuthServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl scouter_tonic::AuthService for AuthServiceImpl {
    #[instrument(skip_all)]
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let login_req = request.into_inner();

        debug!("gRPC login request for user: {}", login_req.username);

        // Get user from database
        let mut user = get_user(&self.state, &login_req.username)
            .await
            .map_err(|e| {
                error!("Failed to get user for login: {:?}", e);
                Status::not_found(format!("User not found: {:?}", e))
            })?;

        // Validate password
        self.state
            .auth_manager
            .validate_user(&user, &login_req.password)
            .map_err(|e| {
                error!("Failed to validate user: {}", e);
                Status::unauthenticated(format!("Invalid credentials: {}", e))
            })?;

        debug!("User {} validated successfully", login_req.username);

        // Generate JWT token
        let jwt_token = self.state.auth_manager.generate_jwt(&user);

        // Check if refresh token is valid, generate new one if needed
        if let Some(refresh_token) = &user.refresh_token {
            if self
                .state
                .auth_manager
                .validate_refresh_token(refresh_token)
                .is_ok()
            {
                return Ok(Response::new(LoginResponse {
                    token: jwt_token,
                    status: "success".to_string(),
                    message: "Login successful".to_string(),
                }));
            }
        }

        let refresh_token = self.state.auth_manager.generate_refresh_token(&user);
        user.refresh_token = Some(refresh_token);

        // Update user in database
        PostgresClient::update_user(&self.state.db_pool, &user)
            .await
            .map_err(|e| {
                error!("Failed to update user refresh token: {}", e);
                Status::internal(format!("Failed to update refresh token: {}", e))
            })?;

        Ok(Response::new(LoginResponse {
            token: jwt_token,
            status: "success".to_string(),
            message: "Login successful".to_string(),
        }))
    }

    #[instrument(skip_all)]
    async fn refresh_token(
        &self,
        request: Request<RefreshTokenRequest>,
    ) -> Result<Response<RefreshTokenResponse>, Status> {
        let refresh_req = request.into_inner();

        debug!("gRPC refresh token request");

        // Decode JWT without full validation to get username
        let claims = self
            .state
            .auth_manager
            .decode_jwt_without_validation(&refresh_req.refresh_token)
            .map_err(|e| Status::unauthenticated(format!("Invalid token: {}", e)))?;

        // Get user from database
        let mut user = get_user(&self.state, &claims.sub).await.map_err(|e| {
            error!("Failed to get user for token refresh: {:?}", e);
            Status::not_found(format!("User not found: {:?}", e))
        })?;

        // Generate new tokens
        let jwt_token = self.state.auth_manager.generate_jwt(&user);
        let refresh_token = self.state.auth_manager.generate_refresh_token(&user);

        user.refresh_token = Some(refresh_token);

        // Update user in database
        PostgresClient::update_user(&self.state.db_pool, &user)
            .await
            .map_err(|e| Status::internal(format!("Failed to update refresh token: {}", e)))?;

        Ok(Response::new(RefreshTokenResponse {
            token: jwt_token,
            status: "success".to_string(),
            message: "Token refreshed successfully".to_string(),
        }))
    }

    #[instrument(skip_all)]
    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>,
    ) -> Result<Response<ValidateTokenResponse>, Status> {
        let validate_req = request.into_inner();

        debug!("gRPC validate token request");

        let is_valid = self
            .state
            .auth_manager
            .validate_jwt(&validate_req.token)
            .is_ok();

        Ok(Response::new(ValidateTokenResponse {
            is_authenticated: is_valid,
            status: if is_valid { "success" } else { "failed" }.to_string(),
            message: if is_valid {
                "Token is valid".to_string()
            } else {
                "Token is invalid".to_string()
            },
        }))
    }
}
