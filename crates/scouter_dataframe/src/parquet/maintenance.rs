use crate::error::TraceEngineError;
use scouter_settings::storage::{
    trace_unsafe_vacuum_allow_zero_from_env, trace_vacuum_retention_hours_from_env,
};

pub fn trace_vacuum_retention_hours() -> u64 {
    trace_vacuum_retention_hours_from_env()
}

pub fn validate_trace_vacuum_retention(retention_hours: u64) -> Result<u64, TraceEngineError> {
    if retention_hours == 0 && !trace_unsafe_vacuum_allow_zero_from_env() {
        return Err(TraceEngineError::UnsupportedOperation(
            "VACUUM retention_hours=0 requires SCOUTER_TRACE_UNSAFE_VACUUM_ALLOW_ZERO=true"
                .to_string(),
        ));
    }

    Ok(retention_hours)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacuum_zero_is_rejected_by_default() {
        if trace_unsafe_vacuum_allow_zero_from_env() {
            return;
        }

        let err = validate_trace_vacuum_retention(0).unwrap_err();
        assert!(matches!(err, TraceEngineError::UnsupportedOperation(_)));
    }

    #[test]
    fn nonzero_vacuum_retention_is_accepted() {
        assert_eq!(validate_trace_vacuum_retention(24).unwrap(), 24);
    }
}
