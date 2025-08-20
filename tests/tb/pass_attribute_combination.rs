use context_variants::variants;
use serde::{Deserialize, Serialize, Deserializer};

// Test to verify global optional_attrs work with field-specific when_optional
#[variants(
    Create: requires(name, email).optional(age).excludes(id),
    Update: requires(id).optional(name, email, age),
    // Global attributes applied to ALL optional fields
    optional_attrs = [serde(skip_serializing_if = "Option::is_none"), serde(default)],
    suffix = "Request"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    
    // Field-specific when_optional attribute for custom deserialization
    // This should work TOGETHER with global optional_attrs
    #[when_optional(serde(deserialize_with = "deserialize_string_to_u64"))]
    pub age: u64,
}

// Custom deserializer that converts string numbers to u64 for optional fields
fn deserialize_string_to_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    
    let opt_str: Option<String> = Option::deserialize(deserializer)?;
    match opt_str {
        Some(s) => {
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse::<u64>()
                    .map(Some)
                    .map_err(|e| D::Error::custom(format!("Failed to parse '{}' as u64: {}", s, e)))
            }
        }
        None => Ok(None),
    }
}

fn main() {
    // Test 1: CreateRequest - age is optional, should get both global and field-specific attrs
    let create_req = CreateRequest {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        age: Some(30), // Has value, should serialize
    };

    let create_req_none = CreateRequest {
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        age: None, // None value, should be skipped due to global skip_serializing_if
    };

    // Test serialization with Some value - should include age
    let json1 = serde_json::to_string(&create_req).unwrap();
    assert!(json1.contains("\"age\":30"), "Should serialize Some(30) age, got: {}", json1);
    
    // Test serialization with None value - should skip age due to skip_serializing_if
    let json2 = serde_json::to_string(&create_req_none).unwrap();
    assert!(!json2.contains("age"), "Should skip None age due to skip_serializing_if, got: {}", json2);

    // Test 2: UpdateRequest - all fields optional except id
    let update_req = UpdateRequest {
        id: 123,
        name: Some("Charlie".to_string()),
        email: None, // Should be skipped
        age: None,   // Should be skipped
    };

    let json3 = serde_json::to_string(&update_req).unwrap();
    assert!(!json3.contains("email"), "Should skip None email, got: {}", json3);
    assert!(!json3.contains("age"), "Should skip None age, got: {}", json3);
    assert!(json3.contains("\"name\":\"Charlie\""), "Should include Some name, got: {}", json3);

    // Test 3: Custom deserializer - JSON with string age that should be converted to u64
    let json_with_string_age = r#"{"name":"David","email":"david@example.com","age":"25"}"#;
    let parsed = serde_json::from_str::<CreateRequest>(json_with_string_age).unwrap();
    assert_eq!(parsed.age, Some(25), "Should parse string '25' to Some(25)");
    assert_eq!(parsed.name, "David");
    assert_eq!(parsed.email, "david@example.com");

    // Test 4: Test with null age (should use default from global attrs)
    let json_with_null_age = r#"{"name":"Eve","email":"eve@example.com","age":null}"#;
    let parsed2 = serde_json::from_str::<CreateRequest>(json_with_null_age).unwrap();
    assert_eq!(parsed2.age, None, "Should parse null age to None");
    assert_eq!(parsed2.name, "Eve");
    assert_eq!(parsed2.email, "eve@example.com");

    // Test 5: Comprehensive test cases
    let test_cases = vec![
        (r#"{"name":"Test1","email":"test1@example.com"}"#, "Missing age field (should default to None)", None),
        (r#"{"name":"Test2","email":"test2@example.com","age":"42"}"#, "String age (should parse to u64)", Some(42)),
        (r#"{"name":"Test3","email":"test3@example.com","age":null}"#, "Null age (should be None)", None),
    ];

    for (json, _description, expected_age) in test_cases {
        let req = serde_json::from_str::<CreateRequest>(json).unwrap();
        assert_eq!(req.age, expected_age, "Age parsing failed for: {}", json);
        
        // Re-serialize to test skip_serializing_if
        let reserialized = serde_json::to_string(&req).unwrap();
        if expected_age.is_none() {
            assert!(!reserialized.contains("age"), "None age should be skipped in reserialization: {}", reserialized);
        } else {
            assert!(reserialized.contains("age"), "Some age should be included in reserialization: {}", reserialized);
        }
    }

    // Test 6: Verify global optional_attrs work with UpdateRequest
    let update_with_none = UpdateRequest {
        id: 999,
        name: None,
        email: None,
        age: None,
    };
    let update_json = serde_json::to_string(&update_with_none).unwrap();
    assert!(!update_json.contains("name"), "None name should be skipped");
    assert!(!update_json.contains("email"), "None email should be skipped");
    assert!(!update_json.contains("age"), "None age should be skipped");
    assert!(update_json.contains("\"id\":999"), "Required id should be included");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_and_field_specific_attributes() {
        // Test that None values are skipped due to global skip_serializing_if
        let req = CreateRequest {
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,
        };
        
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("age"), "None age should be skipped, got: {}", json);
        
        // Test that Some values are included
        let req_with_age = CreateRequest {
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(25),
        };
        
        let json_with_age = serde_json::to_string(&req_with_age).unwrap();
        assert!(json_with_age.contains("age"), "Some age should be included, got: {}", json_with_age);
    }

    #[test] 
    fn test_custom_string_to_u64_deserializer() {
        // Test custom deserializer works with string input
        let json = r#"{"name":"Test","email":"test@example.com","age":"30"}"#;
        let req: CreateRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(req.age, Some(30));
        
        // Test with null
        let json_null = r#"{"name":"Test","email":"test@example.com","age":null}"#;
        let req_null: CreateRequest = serde_json::from_str(json_null).unwrap();
        
        assert_eq!(req_null.age, None);
    }

    #[test]
    fn test_default_behavior() {
        // Test that missing age field defaults to None due to global default
        let json = r#"{"name":"Test","email":"test@example.com"}"#;
        let req: CreateRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(req.age, None);
    }
}
