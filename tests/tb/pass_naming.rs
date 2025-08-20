use context_variants::variants;

// Test prefix and suffix combinations
#[variants(
    Create: requires(name).excludes(id).default(optional),
    Update: requires(id).optional(name).default(exclude),
    prefix = "My",
    suffix = "Entity"
)]
#[derive(Debug, Clone)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

// Test suffix only
#[variants(
    Login: requires(username, password).default(exclude),
    Register: requires(username, password, email).default(exclude),
    suffix = "Form"
)]
#[derive(Debug, Clone)]
struct Auth {
    pub username: String,
    pub password: String,
    pub email: String,
}

// Test prefix only
#[variants(
    Read: requires(id).optional(name).default(exclude),
    Delete: requires(id).default(exclude),
    prefix = "Admin"
)]
#[derive(Debug, Clone)]
struct Operation {
    pub id: u64,
    pub name: String,
}

// Test no prefix or suffix (should use variant name only)
#[variants(
    Simple: requires(value).default(exclude)
)]
#[derive(Debug, Clone)]
struct Base {
    pub value: String,
    pub extra: i32,
}

fn main() {
    // Test prefix + suffix: MyCreateEntity, MyUpdateEntity
    let _create = MyCreateEntity {
        name: "Alice".to_string(),
        email: Some("alice@example.com".to_string()),
    };

    let _update = MyUpdateEntity {
        id: 1,
        name: Some("Alice Updated".to_string()),
    };

    // Test suffix only: LoginForm, RegisterForm
    let _login = LoginForm {
        username: "user".to_string(),
        password: "pass".to_string(),
    };

    let _register = RegisterForm {
        username: "newuser".to_string(),
        password: "newpass".to_string(),
        email: "new@example.com".to_string(),
    };

    // Test prefix only: AdminRead, AdminDelete
    let _read = AdminRead {
        id: 1,
        name: Some("Admin User".to_string()),
    };

    let _delete = AdminDelete {
        id: 2,
    };

    // Test no prefix/suffix: generates variant name only
    let _simple = Simple {
        value: "test".to_string(),
    };
}
