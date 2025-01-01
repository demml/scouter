pub mod health;
pub mod drift;
pub mod profile;
pub mod alerts;
pub mod observability;

pub use health::get_health_router;
pub use drift::get_drift_router;
pub use profile::get_profile_router;
pub use alerts::get_alert_router;
pub use observability::get_observability_router;