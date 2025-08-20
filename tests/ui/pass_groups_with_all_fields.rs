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
    let create = CreateDataRequest {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        metadata: None,
        admin: false,
    };
    println!("CreateDataRequest: name={}, email={}, metadata={:?}, admin={}", 
             create.name, create.email, create.metadata, create.admin);
    
    // UpdateRequest should have id + core_data (name, email) + optional metadata
    let update = UpdateDataRequest {
        id: 1,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        metadata: Some("updated".to_string()),
    };
    println!("UpdateDataRequest: id={}, name={}, email={}, metadata={:?}", 
             update.id, update.name, update.email, update.metadata);
    
    // AdminRequest should have all fields (from all_fields())
    let admin = AdminDataRequest {
        id: 2,
        name: "Charlie".to_string(),
    };
    println!("AdminDataRequest: id={}, name={}", 
             admin.id, admin.name);
    

}
