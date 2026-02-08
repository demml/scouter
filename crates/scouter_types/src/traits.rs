pub trait ScouterRecordExt {
    /// helper for masking sensitive data from the record when
    /// return to the user.
    fn mask_sensitive_data(&mut self);
}

pub trait ConfigExt {
    fn space(&self) -> &str;
    fn name(&self) -> &str;
    fn uid(&self) -> &str;
    fn version(&self) -> &str;
}
