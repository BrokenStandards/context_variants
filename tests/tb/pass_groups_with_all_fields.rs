use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test groups with all_fields() and all_fields().except() support
#[variants(
    groups = (
        all_data(all_fields().except(id)),
        core_data(all_fields().except(id, metadata, admin))
    ),
    Create: requires(all_data).excludes(id).default(exclude),
    Update: requires(id, core_data).optional(metadata).excludes(admin).default(exclude),
    Admin: requires(id,core_data.except(email)).default(exclude),
    suffix = "DataRequest"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Data {
    id: u64,
    name: String,
    email: String,
    metadata: Option<String>,
    admin: bool,
}



fn main() {
    // Test first example: Data with all_fields() groups
    
    // CreateRequest should have all fields except id (from all_data group)
    let _create = CreateDataRequest {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        metadata: None,
        admin: false,
    };
    
    // UpdateRequest should have id + core_data (name, email) + optional metadata
    let _update = UpdateDataRequest {
        id: 1,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        metadata: Some("updated".to_string()),
    };
    
    // AdminRequest should have all fields (from all_fields())
    let _admin = AdminDataRequest {
        id: 2,
        name: "Charlie".to_string(),
    };
}
