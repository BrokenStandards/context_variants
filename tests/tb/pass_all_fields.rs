use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test all_fields().except() syntax
#[variants(
    Create: requires(name, email).excludes(id, admin, password).default(optional),
    Update: requires(id).optional(all_fields().except(password, admin, id)).default(exclude),
    Read: requires(id).optional(all_fields().except(password, id)).default(exclude),
    suffix = "Model"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserModel {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub password: String,
    pub admin: bool,
    pub metadata: Option<serde_json::Value>,
}

// Test all_fields() with different default behaviors
#[variants(
    CreateAll: requires(name).default(optional), // All other fields become optional
    ExcludeAll: requires(id).default(exclude),   // All other fields are excluded
    RequireAll: optional(metadata).default(required), // All other fields are required
    prefix = "Test"
)]
#[derive(Debug, Clone)]
struct TestStruct {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub metadata: Option<String>,
}

fn main() {
    // Test CreateModel - should have name, email required; id, admin, password excluded; metadata optional (from default)
    let _create_model = CreateModel {
        name: "alice".to_string(),
        email: "alice@example.com".to_string(),
        metadata: None,
    };

    // Test UpdateModel - should have id required; all other fields except password, admin optional
    let _update_model = UpdateModel {
        id: 123,
        name: Some("alice_updated".to_string()),
        email: Some("newemail@example.com".to_string()),
        metadata: None,
    };

    // Test ReadModel - should have id required; all other fields except password optional  
    let _read_model = ReadModel {
        id: 456,
        name: Some("read_user".to_string()),
        email: None,
        admin: Some(true),
        metadata: None,
    };

    // Test default behaviors
    let _create_all = TestCreateAll {
        name: "required".to_string(),
        id: Some(1),
        email: Some("optional@example.com".to_string()),
        metadata: None,
    };

    let _exclude_all = TestExcludeAll {
        id: 2,
    };

    let _require_all = TestRequireAll {
        id: 3,
        name: "required".to_string(),
        email: "required@example.com".to_string(),
        metadata: Some("optional".to_string()),
    };
}
