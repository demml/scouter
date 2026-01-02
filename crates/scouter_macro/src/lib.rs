pub mod error;

#[macro_export]
macro_rules! impl_mask_entity_id {
    ($trait_path:path => $($record_type:ty),+ $(,)?) => {
        $(
            impl $trait_path for $record_type {
                /// Masks sensitive data by removing entity_id
                fn mask_sensitive_data(&mut self) {
                    self.entity_id = None;
                }
            }
        )+
    };
}
