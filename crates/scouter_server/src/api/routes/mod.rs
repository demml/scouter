pub mod alerts;
pub mod drift;
pub mod health;
pub mod observability;
pub mod profile;

pub use alerts::get_alert_router;
pub use drift::get_drift_router;
pub use health::get_health_router;
pub use observability::get_observability_router;
pub use profile::get_profile_router;
