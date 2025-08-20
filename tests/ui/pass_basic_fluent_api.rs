use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test basic fluent API with requires/optional/excludes
#[variants(
    Create: requires(name, email, password).excludes(id, admin),
    Update: requires(id, name).optional(email).excludes(password, admin),
    Read: requires(id).optional(name, email, admin).excludes(password),
    suffix = "Form"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserForm {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub password: String,
    pub admin: bool,
}

fn main() {
    // Test CreateForm - should have name, email, password required; exclude id, admin
    let _create_form = CreateForm {
        name: "alice".to_string(),
        email: "alice@example.com".to_string(),
        password: "secure123".to_string(),
    };

    let _json = serde_json::to_string(&_create_form).unwrap();

    // Test UpdateForm - should have id, name required; email optional; exclude password, admin
    let _update_form = UpdateForm {
        id: 123,
        name: "alice_updated".to_string(),
        email: Some("alice_new@example.com".to_string()),
    };

    let _json = serde_json::to_string(&_update_form).unwrap();

    // Test ReadForm - should have id required; name, email, admin optional; exclude password
    let _read_form = ReadForm {
        id: 456,
        name: Some("read_user".to_string()),
        email: None,
        admin: Some(true),
    };

    let _json = serde_json::to_string(&_read_form).unwrap();
}
