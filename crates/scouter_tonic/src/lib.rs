//! Scouter gRPC/Tonic definitions and generated code.
//!
//! This crate provides the protocol buffer definitions and gRPC service
//! implementations for Scouter's message service.
//!
//! ## Features
//!
//! - `server`: Enables server-side gRPC implementation
//! - `client`: Enables client-side gRPC implementation
//!
//! ## Usage
//!
//! ```toml
//! # For server only
//! scouter-tonic = { version = "0.1", features = ["server"] }
//!
//! # For client only
//! scouter-tonic = { version = "0.1", features = ["client"] }
//!
//! # For both
//! scouter-tonic = { version = "0.1", features = ["server", "client"] }
//! ```

// Re-export common types (always available)
pub use generated::scouter::grpc::v1::{
    InsertMessageRequest, InsertMessageResponse, LoginRequest, LoginResponse, RefreshTokenRequest,
    RefreshTokenResponse, ValidateTokenRequest, ValidateTokenResponse,
};

// Re-export client types when feature is enabled
#[cfg(feature = "client")]
pub use generated::scouter::grpc::v1::{
    auth_service_client::AuthServiceClient, message_service_client::MessageServiceClient,
};
#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "client")]
pub use client::GrpcClient;

#[cfg(feature = "client")]
pub mod error;

// Re-export server types when feature is enabled
#[cfg(feature = "server")]
pub use generated::scouter::grpc::v1::{
    auth_service_server::{AuthService, AuthServiceServer},
    message_service_server::{MessageService, MessageServiceServer},
};

mod generated {
    pub mod scouter {
        pub mod grpc {
            pub mod v1 {
                include!("generated/scouter.grpc.v1.rs");
            }
        }
    }
}
