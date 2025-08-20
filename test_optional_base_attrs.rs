use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test that optional_base should apply when_optional and optional_attrs
#[variants(
    Create: requires(name).excludes(id),
    optional_base = true,
    optional_attrs = [serde(skip_serializing_if = "Option::is_none")]
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct User {
    pub id: u64,
    
    #[when_optional(serde(default))]
    #[when_base(doc = "Base struct field")]
    pub name: String,
    
    #[when_optional(serde(rename = "email_addr"))]
    pub email: String,
}

fn main() {
    // This should work and the optional fields in the base struct should have the optional attributes
    let user = User {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: None, // Should skip serialization due to optional_attrs
    };
    
    let json = serde_json::to_string(&user).unwrap();
    println!("JSON: {}", json);
    
    // Test Create variant
    let create = Create {
        name: "Bob".to_string(),
    };
    println!("Create: {:?}", create);
}
