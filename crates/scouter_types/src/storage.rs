use serde::Serialize;

#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum StorageType {
    Google,
    Aws,
    Local,
    Azure,
}
