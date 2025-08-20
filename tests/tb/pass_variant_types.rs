use context_variants::variants;
use serde::{Serialize, Deserialize};

#[variants(
    Request: requires(
        user_id as String,                    // u64 -> String
        action,                             // unchanged
        response as Result<String, ApiError>  // String -> Result<...>
    ).optional(
        metadata as serde_json::Value         // String -> serde_json::Value
    ).excludes(internal_data).default(exclude),
    
    Event: requires(
        user_id,                            // unchanged u64
        timestamp as std::time::SystemTime    // String -> SystemTime
    ).excludes(action, response, metadata, internal_data)
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiEvent {
    pub user_id: u64,
    pub action: String,
    pub response: String,
    pub metadata: String,
    pub timestamp: String,
    pub internal_data: Vec<u8>,
}

// Define a simple error type for the test
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiError {
    message: String,
}

fn main() {
    // Test Request variant with variant types
    let _request = Request {
        user_id: "user123".to_string(),     // String type as specified
        action: "login".to_string(),        // Original String type
        response: Ok("success".to_string()), // Result<String, ApiError> as specified
        metadata: Some(serde_json::json!({"key": "value"})), // serde_json::Value as specified
    };

    // Test Event variant with mixed types
    let _event = Event {
        user_id: 42,                        // Original u64 type (unchanged)
        timestamp: std::time::SystemTime::now(), // SystemTime as specified
    };

    // Test that original struct still works
    let _original = ApiEvent {
        user_id: 42,
        action: "login".to_string(),
        response: "success".to_string(),
        metadata: "some metadata".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        internal_data: vec![1, 2, 3],
    };
}
