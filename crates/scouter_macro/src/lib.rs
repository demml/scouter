pub mod error;

#[macro_export]
macro_rules! extract_str {
    ($out:expr, $field:ident, $value:expr) => {
        if let serde_json::Value::String(s) = $value {
            $out.$field = Some(s.clone());
        }
    };
}

#[macro_export]
macro_rules! extract_i64 {
    ($out:expr, $field:ident, $value:expr) => {
        $out.$field = attr_as_i64($value);
    };
}

#[macro_export]
macro_rules! extract_f64 {
    ($out:expr, $field:ident, $value:expr) => {
        $out.$field = attr_as_f64($value);
    };
}

#[macro_export]
macro_rules! extract_json_str {
    ($out:expr, $field:ident, $value:expr) => {
        $out.$field = value_to_json_string($value);
    };
}

#[macro_export]
macro_rules! or_fallback {
    ($out:expr, $src:expr, $($field:ident),+ $(,)?) => {
        $(
            if $out.$field.is_none() {
                $out.$field = $src.$field.take();
            }
        )+
    };
}

#[macro_export]
macro_rules! append_opt_str {
    ($self:expr, $field:ident, $src:expr) => {
        match &$src {
            Some(v) => $self.$field.append_value(v),
            None => $self.$field.append_null(),
        }
    };
}

#[macro_export]
macro_rules! impl_mask_entity_id {
    ($trait_path:path => $($record_type:ty),+ $(,)?) => {
        $(
            impl $trait_path for $record_type {
                /// Masks sensitive data by removing entity_id
                fn mask_sensitive_data(&mut self) {
                    self.entity_id = -1;
                }
            }
        )+
    };
}
