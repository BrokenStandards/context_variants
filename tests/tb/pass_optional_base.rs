use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test optional_base = true - should make all fields in base struct optional
#[variants(
    CreateRequest: requires(name, email).excludes(id),
    UpdateRequest: requires(id).optional(name, email),
    optional_base = true
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

// Test optional_base = false (explicit) - should keep base struct fields as-is
#[variants(
    Create: requires(title, content).excludes(id),
    Update: requires(id).optional(title, content),
    optional_base = false,
    suffix = "Entity"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Post {
    pub id: u64,
    pub title: String,
    pub content: String,
}

// Test optional_base = true with fields that are already Option<T>
#[variants(
    CreateProfile: requires(username).optional(bio).default(exclude),
    UpdateProfile: requires(id).optional(username, bio),
    optional_base = true,
    suffix = "Data"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Profile {
    pub id: u64,
    pub username: String,
    pub bio: Option<String>, // Already optional
}

fn main() {
    // Test base User struct with optional_base = true
    // All fields should be Option<T> except those already Option<T>
    let _user = User {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: Some("alice@example.com".to_string()),
    };
    
    // Test that we can create with None values
    let _user_partial = User {
        id: None,
        name: Some("Bob".to_string()),
        email: None,
    };
    
    let _json = serde_json::to_string(&_user).unwrap();

    // Test that variant structs work normally
    let _create_req = CreateRequest {
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    };
    
    let _update_req = UpdateRequest {
        id: 123,
        name: Some("Charlie Updated".to_string()),
        email: None,
    };

    // Test base Post struct with optional_base = false
    // Fields should remain as-is (non-optional)
    let _post = Post {
        id: 1,
        title: "Test Post".to_string(),
        content: "This is a test".to_string(),
    };

    // Test base Profile struct with optional_base = true
    // Already optional fields should remain Option<T>, others become Option<T>
    let _profile = Profile {
        id: Some(1),
        username: Some("user123".to_string()),
        bio: Some("A bio".to_string()), // Was already Option<String>
    };
    
    // Test with None values
    let _profile_partial = Profile {
        id: None,
        username: Some("user456".to_string()),
        bio: None, // Can still be None since it was already optional
    };

    // Test variant structs work normally
    let _create_profile = CreateProfileData {
        username: "newuser".to_string(),
        bio: Some("New user bio".to_string()),
    };
    
    let _update_profile = UpdateProfileData {
        id: 456,
        username: Some("updateduser".to_string()),
        bio: None,
    };
}
