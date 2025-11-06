use crate::error::RecordError;
use crate::records::RecordType;

pub trait RecordExt {
    fn record_type(&self) -> Result<RecordType, RecordError>;
}
