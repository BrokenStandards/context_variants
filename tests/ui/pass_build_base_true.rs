use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test build_base = true (explicit) - should generate both base struct and variant structs
#[variants(
    CreateRequest: requires(name, email).excludes(id),
    UpdateRequest: requires(id).optional(name, email),
    build_base = true
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

// Test default behavior (no build_base specified) - should generate both base struct and variant structs
#[variants(
    Create: requires(title, content).excludes(id),
    Update: requires(id).optional(title, content),
    suffix = "Entity"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Post {
    pub id: u64,
    pub title: String,
    pub content: String,
}

fn main() {
    // Test that the base User struct is available when build_base = true
    let _user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    
    let _json = serde_json::to_string(&_user).unwrap();

    // Test that variant structs are also available
    let _create_req = CreateRequest {
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };
    
    let _json = serde_json::to_string(&_create_req).unwrap();

    let _update_req = UpdateRequest {
        id: 123,
        name: Some("Bob Updated".to_string()),
        email: None,
    };
    
    let _json = serde_json::to_string(&_update_req).unwrap();

    // Test that the base Post struct is available when build_base is not specified (defaults to true)
    let _post = Post {
        id: 1,
        title: "My Post".to_string(),
        content: "Post content".to_string(),
    };
    
    let _json = serde_json::to_string(&_post).unwrap();

    // Test that Post variant structs are also available
    let _create_entity = CreateEntity {
        title: "New Post".to_string(),
        content: "New content".to_string(),
    };
    
    let _json = serde_json::to_string(&_create_entity).unwrap();

    let _update_entity = UpdateEntity {
        id: 456,
        title: Some("Updated Post".to_string()),
        content: None,
    };
    
    let _json = serde_json::to_string(&_update_entity).unwrap();
}
