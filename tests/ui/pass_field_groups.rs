use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test basic field groups functionality
#[variants(
    groups = (
        auth(user_id, token),
        contact(name, email)
    ),
    Login: requires(auth).default(exclude),
    Register: requires(contact).optional(auth).default(exclude),
    Update: requires(auth, name).default(exclude),
    prefix = "User"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Request {
    user_id: String,
    token: String,
    name: String,
    email: String,
    metadata: Option<String>,
}

// Test single group definition
#[variants(
    groups = auth(username, password),
    Login: requires(auth).default(exclude),
    suffix = "Form"
)]
#[derive(Debug, Clone)]
struct Auth {
    username: String,
    password: String,
    remember_me: bool,
}

// Test mixed group and individual field usage
#[variants(
    groups = (
        identity(id, name),
        contact(email, phone)
    ),
    Create: requires(identity, email).optional(phone).default(exclude),
    Update: requires(id).optional(contact, name).default(exclude),
    Read: requires(identity).optional(contact).default(exclude),
    suffix = "Data"
)]
#[derive(Debug, Clone)]
struct User {
    id: u64,
    name: String,
    email: String,
    phone: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

fn main() {
    // Test Login with auth group
    let login = UserLogin {
        user_id: "123".to_string(),
        token: "abc123".to_string(),
    };
    println!("Login: {:?}", login);

    // Test Register with contact required and auth optional
    let register = UserRegister {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        user_id: Some("456".to_string()),
        token: Some("def456".to_string()),
    };
    println!("Register: {:?}", register);

    // Test Update with auth group + individual field
    let update = UserUpdate {
        user_id: "789".to_string(),
        token: "ghi789".to_string(),
        name: "Bob".to_string(),
    };
    println!("Update: {:?}", update);

    // Test single group
    let auth_login = LoginForm {
        username: "user".to_string(),
        password: "pass".to_string(),
    };
    println!("Auth Login: {:?}", auth_login);

    // Test mixed usage
    let create_user = CreateData {
        id: 1,
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
        phone: Some("+1234567890".to_string()),
    };
    println!("Create User: {:?}", create_user);

    let update_user = UpdateData {
        id: 1,
        email: Some("charlie.updated@example.com".to_string()),
        phone: None,
        name: Some("Charles".to_string()),
    };
    println!("Update User: {:?}", update_user);

    let read_user = ReadData {
        id: 1,
        name: "Charles".to_string(),
        email: Some("charles@example.com".to_string()),
        phone: Some("+0987654321".to_string()),
    };
    println!("Read User: {:?}", read_user);
}
