pub trait ScouterRecordExt {
    /// helper for masking sensitive data from the record when
    /// return to the user.
    fn mask_sensitive_data(&mut self);
}
