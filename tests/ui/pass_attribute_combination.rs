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
    println!("=== Testing Global optional_attrs + Field-specific when_optional ===\n");

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

    println!("CreateRequest with age:");
    let json1 = serde_json::to_string(&create_req).unwrap();
    println!("  Serialized: {}", json1);
    
    println!("\nCreateRequest without age (should skip age field):");
    let json2 = serde_json::to_string(&create_req_none).unwrap();
    println!("  Serialized: {}", json2);

    // Test 2: UpdateRequest - all fields optional except id
    let update_req = UpdateRequest {
        id: 123,
        name: Some("Charlie".to_string()),
        email: None, // Should be skipped
        age: None,   // Should be skipped
    };

    println!("\nUpdateRequest with partial fields (should skip None fields):");
    let json3 = serde_json::to_string(&update_req).unwrap();
    println!("  Serialized: {}", json3);

    // Test 3: Test custom deserialization with string-to-u64 conversion
    println!("\n=== Testing Custom Deserialization ===");
    
    // JSON with string age that should be converted to u64
    let json_with_string_age = r#"{"name":"David","email":"david@example.com","age":"25"}"#;
    let parsed: Result<CreateRequest, _> = serde_json::from_str(json_with_string_age);
    
    match parsed {
        Ok(req) => {
            println!("✅ Successfully parsed string age to u64:");
            println!("  Parsed: {:?}", req);
            println!("  Age value: {:?}", req.age);
        }
        Err(e) => {
            println!("❌ Failed to parse: {}", e);
        }
    }

    // Test 4: Test with null age (should use default from global attrs)
    let json_with_null_age = r#"{"name":"Eve","email":"eve@example.com","age":null}"#;
    let parsed2: Result<CreateRequest, _> = serde_json::from_str(json_with_null_age);
    
    match parsed2 {
        Ok(req) => {
            println!("\n✅ Successfully handled null age (global default):");
            println!("  Parsed: {:?}", req);
            println!("  Age value: {:?}", req.age);
        }
        Err(e) => {
            println!("\n❌ Failed to parse null age: {}", e);
        }
    }

    // Test 5: Verify both attributes are applied
    println!("\n=== Verification ===");
    
    // This should demonstrate that:
    // 1. Global skip_serializing_if works (None values skipped)
    // 2. Global default works (missing fields get None)  
    // 3. Field-specific when_optional custom deserializer works
    // 4. All attributes work together without conflicts
    
    let test_cases = vec![
        (r#"{"name":"Test1","email":"test1@example.com"}"#, "Missing age field (should default to None)"),
        (r#"{"name":"Test2","email":"test2@example.com","age":"42"}"#, "String age (should parse to u64)"),
        (r#"{"name":"Test3","email":"test3@example.com","age":null}"#, "Null age (should be None)"),
    ];

    for (json, description) in test_cases {
        println!("\nTest: {}", description);
        println!("  Input JSON: {}", json);
        
        match serde_json::from_str::<CreateRequest>(json) {
            Ok(req) => {
                println!("  ✅ Parsed: {:?}", req);
                
                // Re-serialize to test skip_serializing_if
                let reserialized = serde_json::to_string(&req).unwrap();
                println!("  Re-serialized: {}", reserialized);
                
                // Check if None values are properly skipped
                if req.age.is_none() && !reserialized.contains("age") {
                    println!("  ✅ None age correctly skipped in serialization");
                } else if req.age.is_some() && reserialized.contains("age") {
                    println!("  ✅ Some age correctly included in serialization");
                }
            }
            Err(e) => {
                println!("  ❌ Parse error: {}", e);
            }
        }
    }

    println!("\n=== Test Complete ===");
    println!("This test verifies that:");
    println!("1. Global optional_attrs [skip_serializing_if, default] work");
    println!("2. Field-specific when_optional custom deserializer works");
    println!("3. Both attributes combine correctly without conflicts");
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
