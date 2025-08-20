use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test build_base = false - should only generate variant structs, not the base struct
#[variants(
    CreateRequest: requires(name, email).excludes(id),
    UpdateRequest: requires(id).optional(name, email),
    ReadResponse: requires(id, name, email),
    build_base = false
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

// Test with prefix/suffix and build_base = false
#[variants(
    Create: requires(title, content).excludes(id, created_at),
    Update: requires(id).optional(title, content).excludes(created_at),
    Read: requires(id).optional(title, content, created_at),
    prefix = "Blog",
    suffix = "Dto",
    build_base = false
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Post {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub created_at: String,
}

// Test with default behavior and build_base = false
#[variants(
    Simple: requires(name).default(exclude),
    Complex: requires(name, description).optional(metadata).default(exclude),
    build_base = false
)]
#[derive(Debug, Clone)]
struct Config {
    pub name: String,
    pub description: String,
    pub metadata: Option<String>,
    pub internal_flag: bool,
}

fn main() {
    // Test CreateRequest - name, email required; id excluded
    let create_req = CreateRequest {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    
    let _json = serde_json::to_string(&create_req).unwrap();

    // Test UpdateRequest - id required; name, email optional
    let update_req = UpdateRequest {
        id: 123,
        name: Some("Alice Updated".to_string()),
        email: None,
    };
    
    let _json = serde_json::to_string(&update_req).unwrap();

    // Test ReadResponse - id, name, email all required
    let read_resp = ReadResponse {
        id: 456,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };
    
    let _json = serde_json::to_string(&read_resp).unwrap();

    // Test Blog DTOs with prefix/suffix
    let blog_create = BlogCreateDto {
        title: "My Blog Post".to_string(),
        content: "This is the content...".to_string(),
    };
    
    let _json = serde_json::to_string(&blog_create).unwrap();

    let blog_update = BlogUpdateDto {
        id: 789,
        title: Some("Updated Title".to_string()),
        content: None,
    };
    
    let _json = serde_json::to_string(&blog_update).unwrap();

    let blog_read = BlogReadDto {
        id: 101,
        title: Some("Read Title".to_string()),
        content: Some("Read content".to_string()),
        created_at: Some("2023-01-01".to_string()),
    };
    
    let _json = serde_json::to_string(&blog_read).unwrap();

    // Test Config variants
    let _simple_config = Simple {
        name: "simple".to_string(),
    };

    let _complex_config = Complex {
        name: "complex".to_string(),
        description: "A complex config".to_string(),
        metadata: Some("extra data".to_string()),
    };

    // Note: The original User, Post, and Config structs should NOT be available
    // when build_base = false. If they were available, this would be a compilation error.
    
    // This would fail to compile if build_base = false works correctly:
    // let _user = User { id: 1, name: "test".to_string(), email: "test@example.com".to_string() };
    // let _post = Post { id: 1, title: "test".to_string(), content: "test".to_string(), created_at: "now".to_string() };
    // let _config = Config { name: "test".to_string(), description: "test".to_string(), metadata: None, internal_flag: true };
}
