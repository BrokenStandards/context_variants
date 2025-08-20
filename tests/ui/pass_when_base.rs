use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test when_base functionality
#[variants(
    Create: requires(name, email).excludes(id),
    Update: requires(id, name).optional(email),
    suffix = "Request"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct User {
    #[when_base(serde(rename = "database_id"))]
    pub id: u64,

    #[when_base(serde(deserialize_with = "deserialize_full_name"))]
    #[when_required(serde(rename = "full_name"))]
    pub name: String,

    #[when_base(serde(deserialize_with = "deserialize_complete_email"))]
    #[when_optional(serde(skip_serializing_if = "Option::is_none"))]
    #[when_required(serde(rename = "email_addr"))]
    pub email: String,
}

// Mock deserializer functions
fn deserialize_full_name<'de, D>(_deserializer: D) -> Result<String, D::Error>
where D: serde::Deserializer<'de> {
    Ok("full_name_processed".to_string())
}

fn deserialize_complete_email<'de, D>(_deserializer: D) -> Result<String, D::Error>
where D: serde::Deserializer<'de> {
    Ok("complete_email_processed".to_string())
}

fn main() {
    // Test base struct - should have when_base attributes
    let base_user = User {
        id: 123,  // Should be serialized as "database_id"
        name: "John Doe".to_string(),  // Should use deserialize_full_name
        email: "john@example.com".to_string(),  // Should use deserialize_complete_email
    };

    // Test CreateRequest - should have when_required attributes but NOT when_base
    let create_req = CreateRequest {
        name: "Jane".to_string(),  // Should be serialized as "full_name" (when_required)
        email: "jane@example.com".to_string(),  // Should be serialized as "email_addr" (when_required)
    };

    // Test UpdateRequest - should have mixed attributes
    let update_req = UpdateRequest {
        id: 456,  // Should NOT have when_base attributes (no "database_id" rename)
        name: "Updated".to_string(),  // Should be serialized as "full_name" (when_required)
        email: Some("updated@example.com".to_string()),  // Should use skip_serializing_if (when_optional)
    };

    // Test serialization
    let base_json = serde_json::to_string(&base_user).unwrap();
    let create_json = serde_json::to_string(&create_req).unwrap();
    let update_json = serde_json::to_string(&update_req).unwrap();

    println!("Base User JSON (should show when_base attributes):");
    println!("{}\n", base_json);

    println!("CreateRequest JSON (should show when_required attributes):");
    println!("{}\n", create_json);

    println!("UpdateRequest JSON (should show when_optional/when_required attributes):");
    println!("{}\n", update_json);

    // Verify when_base attributes are only on base struct
    assert!(base_json.contains("\"database_id\":123"));  // when_base rename
    assert!(!create_json.contains("database_id"));  // Not on variants
    assert!(!update_json.contains("database_id"));  // Not on variants

    // Verify when_required attributes work on variants
    assert!(create_json.contains("\"full_name\":\"Jane\""));
    assert!(create_json.contains("\"email_addr\":\"jane@example.com\""));
    assert!(update_json.contains("\"full_name\":\"Updated\""));

    println!("✅ when_base functionality working correctly!");
}
