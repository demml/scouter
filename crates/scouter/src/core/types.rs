use crate::core::utils::AlertDispatchType;

pub trait AlertFeatures {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String;
}
