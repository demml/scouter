use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserPermissions {
    pub username: String,
    pub permissions: Vec<String>,
    pub group_permissions: Vec<String>,
}

impl UserPermissions {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
            || self.group_permissions.contains(&"admin".to_string())
    }

    pub fn has_read_permission(&self) -> bool {
        self.has_permission("read")
    }

    pub fn has_write_permission(&self, space_id: &str) -> bool {
        self.has_permission(&format!("write:{space_id}"))
            || self.permissions.contains(&"write:all".to_string())
    }

    pub fn has_delete_permission(&self, space_id: &str) -> bool {
        self.has_permission(&format!("delete:{space_id}"))
    }
}
