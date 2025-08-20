use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test context-level attribute configuration
#[variants(
    Create: requires(name, email, password).excludes(id, admin, metadata),
    Update: requires(id, name, admin).optional(email, metadata).excludes(password),
    Read: requires(id, admin).optional(name, email, metadata).excludes(password),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none"), serde(default)],
    suffix = "Entity"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserEntity {
    /// User ID field
    #[serde(rename = "user_id")]
    pub id: u64,

    /// Username field
    #[serde(rename = "username")]
    pub name: String,

    /// Email field with when_* attributes
    #[when_base(serde(deserialize_with = "deserialize_email_base"))]
    #[when_optional(serde(deserialize_with = "deserialize_email_optional"))]
    #[when_required(serde(deserialize_with = "deserialize_email_required"))]
    #[serde(rename = "email_address")]
    pub email: String,

    /// Password field
    #[serde(rename = "password")]
    pub password: String,

    /// Admin flag
    #[serde(rename = "is_admin", default)]
    pub admin: bool,

    /// Metadata field
    #[serde(rename = "meta")]
    pub metadata: Option<serde_json::Value>,
}

// Mock deserializer functions for testing
fn deserialize_email_base<'de, D>(_deserializer: D) -> Result<String, D::Error>
where D: serde::Deserializer<'de> {
    Ok("base@example.com".to_string())
}

fn deserialize_email_optional<'de, D>(_deserializer: D) -> Result<Option<String>, D::Error>
where D: serde::Deserializer<'de> {
    Ok(Some("optional@example.com".to_string()))
}

fn deserialize_email_required<'de, D>(_deserializer: D) -> Result<String, D::Error>
where D: serde::Deserializer<'de> {
    Ok("required@example.com".to_string())
}

fn main() {
    // Test CreateEntity - should have required_attrs applied to required fields
    let create_entity = CreateEntity {
        name: "alice".to_string(),
        email: "alice@example.com".to_string(),
        password: "secure123".to_string(),
    };

    let json = serde_json::to_string(&create_entity).unwrap();
    println!("CreateEntity JSON: {}", json);

    // Test UpdateEntity - should have optional_attrs applied to optional fields
    let update_entity = UpdateEntity {
        id: 123,
        name: "alice_updated".to_string(),
        email: None, // Should use optional_attrs (skip_serializing_if, default)
        admin: false,
        metadata: Some(serde_json::json!({"updated": true})),
    };

    let json = serde_json::to_string(&update_entity).unwrap();
    println!("UpdateEntity JSON: {}", json);

    // Test ReadEntity - mix of required and optional fields
    let read_entity = ReadEntity {
        id: 456,
        admin: true,
        name: Some("read_user".to_string()),
        email: None, // Should be skipped due to optional_attrs
        metadata: None, // Should be skipped due to optional_attrs
    };

    let json = serde_json::to_string(&read_entity).unwrap();
    println!("ReadEntity JSON: {}", json);
}
