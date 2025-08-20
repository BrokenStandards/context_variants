use context_variants::variants;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, skip_serializing_none};
use serde_json;

#[variants(
    suffix = "Request",
    optional_base = true,
    Received: requires(id).default(optional),
)]
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Request {
    // Basic identification
    #[when_required(serde_as(as = "DisplayFromStr"))]
    #[when_optional(serde_as(as = "Option<DisplayFromStr>"))]
    #[when_base(serde_as(as = "Option<DisplayFromStr>"))]
    pub id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_request_serialization() {
        // Test base Request with Some(id)
        let request = Request {
            id: Some(42),
        };
        
        let json = serde_json::to_string(&request).expect("Failed to serialize Request");
        println!("Base Request JSON: {}", json);
        
        // Should serialize id as string due to DisplayFromStr
        assert!(json.contains(r#""id":"42""#));
    }

    #[test]
    fn test_base_request_deserialization() {
        // Test deserializing from string representation
        let json = r#"{"id":"123"}"#;
        let request: Request = serde_json::from_str(json).expect("Failed to deserialize Request");
        
        assert_eq!(request.id, Some(123));
    }

    #[test]
    fn test_base_request_none_serialization() {
        // Test base Request with None (should be skipped due to skip_serializing_none)
        let request = Request {
            id: None,
        };
        
        let json = serde_json::to_string(&request).expect("Failed to serialize Request");
        println!("Base Request with None JSON: {}", json);
        
        // Should not contain id field due to skip_serializing_none
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_base_request_default() {
        // Test default construction
        let request = Request::default();
        assert_eq!(request.id, None);
        
        // Test serde default behavior
        let json = "{}";
        let request: Request = serde_json::from_str(json).expect("Failed to deserialize empty Request");
        assert_eq!(request.id, None);
    }

    #[test]
    fn test_received_request_serialization() {
        // Test ReceivedRequest variant (requires id)
        let received = ReceivedRequest {
            id: 999,
        };
        
        let json = serde_json::to_string(&received).expect("Failed to serialize ReceivedRequest");
        println!("ReceivedRequest JSON: {}", json);
        
        // Should serialize id as string due to DisplayFromStr
        assert!(json.contains(r#""id":"999""#));
    }

    #[test]
    fn test_received_request_deserialization() {
        // Test deserializing ReceivedRequest from string representation
        let json = r#"{"id":"456"}"#;
        let received: ReceivedRequest = serde_json::from_str(json).expect("Failed to deserialize ReceivedRequest");
        
        assert_eq!(received.id, 456);
    }

    #[test]
    fn test_received_request_invalid_id() {
        // Test that invalid string fails to deserialize
        let json = r#"{"id":"not_a_number"}"#;
        let result = serde_json::from_str::<ReceivedRequest>(json);
        
        assert!(result.is_err(), "Should fail to deserialize invalid number string");
    }

    #[test]
    fn test_display_from_str_conversion() {
        // Test that very large numbers work correctly
        let large_id = u64::MAX;
        let received = ReceivedRequest {
            id: large_id,
        };
        
        let json = serde_json::to_string(&received).expect("Failed to serialize large ID");
        let deserialized: ReceivedRequest = serde_json::from_str(&json).expect("Failed to deserialize large ID");
        
        assert_eq!(deserialized.id, large_id);
    }

    #[test]
    fn test_partial_json_base_request() {
        // Test that missing fields use default values in base Request
        let json = r#"{}"#;
        let request: Request = serde_json::from_str(json).expect("Failed to deserialize partial Request");
        
        assert_eq!(request.id, None);
    }

    #[test]
    fn test_extra_fields_ignored() {
        // Test that extra fields are ignored due to serde(default)
        let json = r#"{"id":"789","extra_field":"ignored"}"#;
        let request: Request = serde_json::from_str(json).expect("Failed to deserialize Request with extra fields");
        
        assert_eq!(request.id, Some(789));
    }
}
