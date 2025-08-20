use context_variants::{context_variants, variants};
use serde::{Deserialize, Serialize};

// Test the current (working) syntax first
#[context_variants(Create, Update, Read, suffix = "Request")]
#[ctx_default_optional(Create, Update, Read)]  // By default, all fields are optional unless overridden
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserRequest {
    /// User ID is required for Update and Read operations
    #[ctx_required(Update, Read)]
    #[ctx_optional_attr(serde(skip_serializing_if = "Option::is_none"))]
    #[serde(rename = "user_id")]
    pub id: u64,

    /// Username is required for Create and Update operations
    #[ctx_required(Create, Update)]
    #[ctx_optional_attr(serde(skip_serializing_if = "Option::is_none"))]
    #[serde(rename = "username")]
    pub name: String,

    /// Email is required for Create, optional for Update
    #[ctx_required(Create)]
    #[ctx_optional_attr(serde(skip_serializing_if = "Option::is_none"))]
    #[serde(rename = "email_address")]
    pub email: String,

    /// Password is only used during creation
    #[ctx_required(Create)]
    #[ctx_never(Update, Read)]
    #[serde(rename = "password")]
    pub password: String,

    /// Metadata is optional in all variants
    #[serde(rename = "meta", skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,

    /// Admin flag is never included in create requests (security)
    #[ctx_never(Create)]
    #[ctx_required(Update, Read)]
    #[serde(rename = "is_admin", default)]
    pub admin: bool,

    /// Timestamp fields with custom serde attributes
    #[serde(rename = "created_at", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    #[serde(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}


#[test]
fn test_create_request_serde() {
    // CreateRequest should have: name (required), email (required), password (required), 
    // metadata (optional), created_at (optional), updated_at (optional)
    // id (optional), admin field completely missing due to ctx_never
    let create_req = CreateRequest {
        id: None, // Optional in Create variant
        name: "alice".to_string(),
        email: "alice@example.com".to_string(),
        password: "secret123".to_string(),
        metadata: Some(serde_json::json!({"source": "api"})),
        created_at: None,
        updated_at: None,
    };

    // Test serialization
    let json = serde_json::to_string(&create_req).unwrap();
    println!("CreateRequest JSON: {}", json);
    
    // Should contain renamed fields
    assert!(json.contains("\"username\":\"alice\""));
    assert!(json.contains("\"email_address\":\"alice@example.com\""));
    assert!(json.contains("\"password\":\"secret123\""));
    assert!(json.contains("\"meta\":{\"source\":\"api\"}"));
    
    // Should NOT contain id or admin fields
    assert!(!json.contains("user_id"));
    assert!(!json.contains("is_admin"));

    // Test deserialization
    let json_input = r#"{
        "username": "bob",
        "email_address": "bob@example.com", 
        "password": "password456",
        "meta": {"role": "user"}
    }"#;
    
    let deserialized: CreateRequest = serde_json::from_str(json_input).unwrap();
    assert_eq!(deserialized.name, "bob");
    assert_eq!(deserialized.email, "bob@example.com");
    assert_eq!(deserialized.password, "password456");
    assert!(deserialized.metadata.is_some());
}

#[test]
fn test_update_request_serde() {
    // UpdateRequest should have: id (required), name (required), email (optional), admin (required),
    // metadata (optional), created_at (optional), updated_at (optional)
    // Missing: password
    let update_req = UpdateRequest {
        id: 123,
        name: "alice_updated".to_string(),
        email: Some("newemail@example.com".to_string()),
        admin: false,
        metadata: None,
        created_at: None,
        updated_at: Some("2023-01-01T00:00:00Z".to_string()),
    };

    // Test serialization
    let json = serde_json::to_string(&update_req).unwrap();
    println!("UpdateRequest JSON: {}", json);
    
    // Should contain renamed fields
    assert!(json.contains("\"user_id\":123"));
    assert!(json.contains("\"username\":\"alice_updated\""));
    assert!(json.contains("\"email_address\":\"newemail@example.com\""));
    assert!(json.contains("\"is_admin\":false"));
    assert!(json.contains("\"updated_at\":\"2023-01-01T00:00:00Z\""));
    
    // Should NOT contain password field
    assert!(!json.contains("password"));
    
    // Should not include null metadata due to skip_serializing_if
    assert!(!json.contains("\"meta\":null"));

    // Test deserialization with missing optional fields
    let json_input = r#"{
        "user_id": 456,
        "username": "charlie",
        "is_admin": false
    }"#;
    
    let deserialized: UpdateRequest = serde_json::from_str(json_input).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "charlie");
    assert_eq!(deserialized.email, None);
    assert_eq!(deserialized.admin, false);
    assert!(deserialized.metadata.is_none());
}

#[test]
fn test_read_request_serde() {
    // ReadRequest should have: id (required), admin (required),
    // name (optional), email (optional), metadata (optional), created_at (optional), updated_at (optional)
    // Missing: password (due to ctx_never)
    let read_req = ReadRequest {
        id: 789,
        name: None, // Optional in Read variant
        email: None, // Optional in Read variant
        admin: true,
        metadata: None,
        created_at: Some("2023-01-01T00:00:00Z".to_string()),
        updated_at: None,
    };

    // Test serialization
    let json = serde_json::to_string(&read_req).unwrap();
    println!("ReadRequest JSON: {}", json);
    
    // Should contain renamed fields
    assert!(json.contains("\"user_id\":789"));
    assert!(json.contains("\"is_admin\":true"));
    assert!(json.contains("\"created_at\":\"2023-01-01T00:00:00Z\""));
    
    // Should NOT contain name, email, or password fields
    assert!(!json.contains("username"));
    assert!(!json.contains("email_address"));
    assert!(!json.contains("password"));

    // Test deserialization
    let json_input = r#"{
        "user_id": 999,
        "is_admin": false
    }"#;
    
    let deserialized: ReadRequest = serde_json::from_str(json_input).unwrap();
    assert_eq!(deserialized.id, 999);
    assert_eq!(deserialized.admin, false);
    assert!(deserialized.metadata.is_none());
    assert!(deserialized.created_at.is_none());
    assert!(deserialized.updated_at.is_none());
}



// Test the new fluent syntax - start simple with just a function call
#[variants(
    Create: requires(name).excludes(email, password,id),
    Update: requires(id, name).optional(email).excludes(password),
    suffix = "Form"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserForm {
    #[serde(rename = "username")]
    pub name: String,
    pub id: u64,
    pub email: String,
    pub password: String,
}

#[test]
fn test_user_form() {
    let user_form = UserForm {
        name: "bob".to_string(),
        id: 1,
        email: "bob@example.com".to_string(),
        password: "password123".to_string(),
    };

    // Test serialization
    let json = serde_json::to_string(&user_form).unwrap();
    println!("UserForm JSON: {}", json);

    // Should contain renamed fields
    assert!(json.contains("\"username\":\"bob\""));

    // Test deserialization
    let deserialized: UserForm = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "bob");
}

#[test]
fn test_create_form() {
    // CreateRequest should have: name (required)
    let create_req = CreateForm {
        name: "alice".to_string(),
    };

    // Test serialization
    let json = serde_json::to_string(&create_req).unwrap();
    println!("CreateForm JSON: {}", json);

    // Should contain renamed fields
    assert!(json.contains("\"username\":\"alice\""));

    // Test deserialization
    let deserialized: CreateForm = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "alice");
}

#[test]
fn test_update_form() {
    // UpdateRequest should have: id (required), name (required), email (optional), password (optional)
    let update_req = UpdateForm {
        id: 1,
        name: "bob".to_string(),
        email: None,
    };

    // Test serialization
    let json = serde_json::to_string(&update_req).unwrap();
    println!("UpdateForm JSON: {}", json);

    // Should contain renamed fields
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"username\":\"bob\""));

    // Test deserialization
    let deserialized: UpdateForm = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, 1);
    assert_eq!(deserialized.name, "bob");
}

// Test method chaining in fluent syntax with defaults
#[variants(
    Create: requires(name, email).default(exclude),
    Update: requires(id, name).optional(email).default(optional).excludes(password,admin), 
    Read: requires(id).optional(name, email).default(exclude),
    default = exclude,
    suffix = "Data"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserData {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub password: String,
    pub metadata: Option<serde_json::Value>,
    pub admin: bool,
}

#[test]
fn test_method_chaining() {
    // Test CreateData - should have name, email required; metadata optional; id, admin excluded
    let create_data = CreateData {
        name: "alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    let json = serde_json::to_string(&create_data).unwrap();
    println!("CreateData JSON: {}", json);
    assert!(json.contains("\"name\":\"alice\""));
    assert!(json.contains("\"email\":\"alice@example.com\""));
    
    // Test UpdateData - should have id, name required; email, metadata optional; password excluded
    let update_data = UpdateData {
        id: 123,
        name: "alice_updated".to_string(),
        email: Some("newemail@example.com".to_string()),
        metadata: None,
    };

    let json = serde_json::to_string(&update_data).unwrap();
    println!("UpdateData JSON: {}", json);
    assert!(json.contains("\"id\":123"));
    assert!(json.contains("\"name\":\"alice_updated\""));
    assert!(json.contains("\"email\":\"newemail@example.com\""));
    
    // Test ReadData - should have id required; name, email, metadata optional; password, admin excluded
    let read_data = ReadData {
        id: 456,
        name: None,
        email: None,
    };

    let json = serde_json::to_string(&read_data).unwrap();
    println!("ReadData JSON: {}", json);
    assert!(json.contains("\"id\":456"));
}

// Test field groups functionality (future enhancement)
// This demonstrates the intended API for field groups
#[test]

fn test_field_groups_syntax() {
    // Test multiple groups definition
    use context_variants::variants;

    #[variants(
        prefix = "UserRequest",
        groups = (
            auth(user_id, token), 
            contact(name, email)
        ),
        Login: requires(auth).default(exclude),
        Register: requires(contact).optional(auth).default(exclude),
        Update: requires(auth, name).default(exclude),
    )]
    #[derive(Debug)]
    struct UserRequest {
        user_id: String,
        token: String,
        name: String,
        email: String,
        metadata: Option<String>,
    }
    
    // Test Login variant - should require auth fields (user_id, token)
    let login = UserRequestLogin {
        user_id: "123".to_string(),
        token: "abc".to_string(),
    };
    
    assert_eq!(login.user_id, "123");
    assert_eq!(login.token, "abc");
    
    // Test Register variant - should require contact fields and optionally auth fields
    let register = UserRequestRegister {
        name: "John".to_string(),
        email: "john@example.com".to_string(),
        user_id: Some("123".to_string()),
        token: Some("abc".to_string()),
    };
    
    assert_eq!(register.name, "John");
    assert_eq!(register.email, "john@example.com");
    assert_eq!(register.user_id, Some("123".to_string()));
    
    // Test Update variant - should require auth + name (mix of group and individual field)
    let update = UserRequestUpdate {
        user_id: "456".to_string(),
        token: "def".to_string(),
        name: "Jane".to_string(),
    };
    
    assert_eq!(update.user_id, "456");
    assert_eq!(update.token, "def");
    assert_eq!(update.name, "Jane");
}

// Test conditional field attributes with when_optional and when_required
#[variants(
    Login: requires(email, password,username).default(exclude),
    Profile: requires(username).optional(email).excludes(password),
    suffix = "Form"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ConditionalForm {
    // Field gets different attributes based on optional vs required
    #[when_optional(serde(skip_serializing_if = "Option::is_none"))]
    #[when_required(serde(rename = "email_address"))]
    pub email: String,
    
    #[when_optional(serde(default))]
    #[when_required(serde(rename = "pwd"))]
    pub password: String,
    
    pub username: String,
}

#[test]
fn test_conditional_attributes() {
    // Test LoginForm - email and password are required, so they get "required" attributes
    let login = LoginForm {
        email: "user@example.com".to_string(),  // Will be serialized as "email_address"
        password: "secret".to_string(),         // Will be serialized as "pwd"
        username: "john".to_string(),
    };
    
    let json = serde_json::to_string(&login).unwrap();
    println!("LoginForm JSON: {}", json);
    
    // Should use required attributes (rename)
    assert!(json.contains("\"email_address\":\"user@example.com\""));
    assert!(json.contains("\"pwd\":\"secret\""));
    assert!(json.contains("\"username\":\"john\""));
    
    // Test ProfileForm - email is optional, password excluded, so email gets "optional" attributes
    let profile = ProfileForm {
        email: Some("user@example.com".to_string()), // Will use skip_serializing_if
        username: "jane".to_string(),
    };
    
    let profile_json = serde_json::to_string(&profile).unwrap();
    println!("ProfileForm JSON: {}", profile_json);
    
    // Should use optional attributes (no rename, but has skip_serializing_if)
    assert!(profile_json.contains("\"email\":\"user@example.com\""));
    assert!(profile_json.contains("\"username\":\"jane\""));
    
    // Test with None value - should skip serialization due to when_optional attribute
    let profile_empty = ProfileForm {
        email: None,
        username: "empty".to_string(),
    };
    
    let empty_json = serde_json::to_string(&profile_empty).unwrap();
    println!("ProfileForm (empty) JSON: {}", empty_json);
    
    // Should not contain email field due to skip_serializing_if
    assert!(!empty_json.contains("email"));
    assert!(empty_json.contains("\"username\":\"empty\""));
}
