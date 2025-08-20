# context_variants

`context_variants` is a Rust procedural macro that generates variant structs from a base struct definition, allowing you to create specialized versions of data structures for different contexts (e.g., Create, Update, Read operations). Each variant can specify which fields are required, optional, or excluded, along with context-specific attributes.

## Quick Start

First, add `context_variants` to your dependencies:

```toml
[dependencies]
context_variants = "0.1.0"
serde = { version = "1.0", features = ["derive"] }
```

Then annotate your struct using the fluent API:

```rust
use context_variants::variants;
use serde::{Serialize, Deserialize};

#[variants(
    Create: requires(name, email).excludes(id, created_at),
    Update: requires(id).optional(name, email).excludes(created_at),
    Read: requires(id, name, email, created_at),
    suffix = "Request"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

This generates three structs: `CreateRequest`, `UpdateRequest`, and `ReadRequest`.

## Understanding Generated Variants

Let's see what the above macro generates. Starting with this base struct:

```rust
#[variants(
    Create: requires(name, email).excludes(id, password).default(optional),
    Update: requires(id).optional(name, email).excludes(password),
    Read: requires(id).optional(name, email, password).default(exclude),
    suffix = "User"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserData {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub password: String,
    pub metadata: Option<serde_json::Value>,
}
```

### Base Struct Remains Unchanged

Your original struct is preserved and works exactly as before:

```rust
let user_data = UserData {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    password: "secret123".to_string(),
    metadata: Some(serde_json::json!({"role": "admin"})),
};
```

### Generated Variant Structs

The macro generates three new structs based on your specifications:

#### CreateUser
From `Create: requires(name, email).excludes(id, password).default(optional)`

```rust
// Generated struct (conceptually):
struct CreateUser {
    pub name: String,           // Required
    pub email: String,          // Required  
    pub metadata: Option<serde_json::Value>, // Optional (from default)
    // id and password fields are excluded
}

// Usage:
let create_user = CreateUser {
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    metadata: Some(serde_json::json!({"source": "signup"})),
};
```

#### UpdateUser  
From `Update: requires(id).optional(name, email).excludes(password)`

```rust
// Generated struct (conceptually):
struct UpdateUser {
    pub id: u64,                // Required
    pub name: Option<String>,   // Optional
    pub email: Option<String>,  // Optional
    pub metadata: Option<serde_json::Value>, // Optional (from default)
    // password field is excluded
}

// Usage:
let update_user = UpdateUser {
    id: 1,
    name: Some("Alice Updated".to_string()),
    email: None, // Not updating email
    metadata: None,
};
```

#### ReadUser
From `Read: requires(id).optional(name, email, password).default(exclude)`

```rust
// Generated struct (conceptually):
struct ReadUser {
    pub id: u64,                    // Required
    pub name: Option<String>,       // Optional
    pub email: Option<String>,      // Optional
    pub password: Option<String>,   // Optional
    // metadata is excluded (from default)
}

// Usage:
let read_user = ReadUser {
    id: 1,
    name: Some("Alice".to_string()),
    email: Some("alice@example.com".to_string()),
    password: None, // Might not include password in response
};
```

## Fluent API Reference

Each variant uses a fluent API with method chaining to specify field behavior:

* `requires(field1, field2, ...)` - Fields that must be present and non-optional
* `optional(field1, field2, ...)` - Fields that become `Option<T>`
* `excludes(field1, field2, ...)` - Fields that are completely omitted from the variant
* `default(behavior)` - Sets default behavior for unspecified fields

### Default Behaviors

You can set default behavior for unspecified fields:

- `default(exclude)` - Unspecified fields are excluded from the variant
- `default(optional)` - Unspecified fields become optional (`Option<T>`)
- `default(required)` - Unspecified fields remain required (no change)

```rust
#[variants(
    Create: requires(name, email).default(exclude),        // Only name, email
    Update: requires(id).default(optional).excludes(password), // id + all others optional except password
    Read: requires(id).optional(name, email).default(exclude)  // id + name, email optional, rest excluded
)]
```

## Real-World Example: REST API

Here's a complete example showing how you might use this for a REST API:

```rust
use context_variants::variants;
use serde::{Serialize, Deserialize};

#[variants(
    CreateUserRequest: requires(username, email, password).excludes(id, created_at, updated_at),
    UpdateUserRequest: requires(id).optional(username, email).excludes(password, created_at, updated_at),
    UserResponse: requires(id, username, email, created_at).optional(updated_at).excludes(password),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none")],
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    #[serde(rename = "user_id")]
    pub id: u64,
    
    pub username: String,
    
    #[serde(rename = "email_address")]
    pub email: String,
    
    pub password: String,
    
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

// Usage in your API handlers:
async fn create_user(request: CreateUserRequest) -> Result<UserResponse, Error> {
    // request has: username, email, password (no id, timestamps)
    let user = User {
        id: generate_id(),
        username: request.username,
        email: request.email,
        password: hash_password(request.password),
        created_at: Utc::now(),
        updated_at: None,
    };
    
    save_user(&user).await?;
    
    // Return response without password
    Ok(UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        created_at: user.created_at,
        updated_at: user.updated_at,
    })
}

async fn update_user(request: UpdateUserRequest) -> Result<UserResponse, Error> {
    // request has: id (required), username, email (optional), no password/timestamps
    let mut user = find_user(request.id).await?;
    
    if let Some(username) = request.username {
        user.username = username;
    }
    if let Some(email) = request.email {
        user.email = email;
    }
    user.updated_at = Some(Utc::now());
    
    save_user(&user).await?;
    
    Ok(UserResponse::from(user))
}
```

## Advanced Features

### Bulk Field Operations with all_fields()

For structs with many fields, use `all_fields().except(...)`:

```rust
#[variants(
    Create: requires(name, email).excludes(id, admin, password).default(optional),
    Update: requires(id).optional(all_fields().except(password, admin, id)).default(exclude),
    Read: requires(id).optional(all_fields().except(password, id)).default(exclude)
)]
struct User {
    id: u64,
    name: String,
    email: String,
    password: String,
    admin: bool,
    metadata: Option<serde_json::Value>,
    preferences: Option<String>,
    last_login: Option<chrono::DateTime<chrono::Utc>>,
}
```

This generates:
- `CreateUser`: requires name, email; optional metadata, preferences, last_login; excludes id, admin, password
- `UpdateUser`: requires id; optional name, email, metadata, preferences, last_login; excludes password, admin
- `ReadUser`: requires id; optional name, email, admin, metadata, preferences, last_login; excludes password

### Field-Level Conditional Attributes

Apply different attributes based on whether a field is required or optional:

```rust
#[variants(
    Login: requires(email, password).default(exclude),
    Profile: requires(username).optional(email).excludes(password)
)]
#[derive(Debug, Serialize, Deserialize)]
struct User {
    #[when_optional(serde(skip_serializing_if = "Option::is_none"))]
    #[when_required(serde(rename = "email_address"))]
    pub email: String,
    
    #[when_required(serde(rename = "pwd"))]
    pub password: String,
    
    pub username: String,
}
```

- In `Login` variant: email serializes as "email_address", password as "pwd"
- In `Profile` variant: email is optional and skips serialization when None

### Context-Level Attributes

Apply attributes to all optional/required fields across variants:

```rust
#[variants(
    Create: requires(name, email).excludes(id),
    Update: requires(id).optional(name, email),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none"), serde(default)],
    required_attrs = [serde(deny_unknown_fields = false)],
    suffix = "Dto"
)]
#[derive(Debug, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}
```

All optional fields automatically get `skip_serializing_if` and `default` attributes.

### Field Groups

Group related fields for easier management:

```rust
#[variants(
    prefix = "UserRequest",
    groups = (
        auth(user_id, token), 
        contact(name, email)
    ),
    Login: requires(auth).default(exclude),
    Register: requires(contact).optional(auth).default(exclude),
    Update: requires(auth, name).default(exclude)
)]
struct UserRequest {
    user_id: String,
    token: String,
    name: String,
    email: String,
    metadata: Option<String>,
}
```

### Naming Configuration

Control generated struct names with prefix and suffix:

```rust
#[variants(
    Create: requires(name),
    Update: requires(id, name),
    prefix = "User",
    suffix = "Request"
)]
struct Data {
    id: u64,
    name: String,
}
```

Generates: `UserCreateRequest`, `UserUpdateRequest`

## Validation and Error Handling

The macro provides compile-time validation:

```rust
// ❌ ERROR: field 'name' specified multiple times
#[variants(Create: requires(name).optional(name))]

// ❌ ERROR: field 'nonexistent' not found
#[variants(Create: requires(nonexistent))]

// ❌ ERROR: field 'email' not covered and no default behavior
#[variants(Create: requires(name))]
struct User { name: String, email: String }

// ✅ OK: default behavior specified  
#[variants(Create: requires(name).default(exclude))]
struct User { name: String, email: String }
```

## Integration with Existing Code

### Serde Compatibility

All serde attributes are preserved and work correctly:

```rust
#[variants(
    CreateDto: requires(name).optional(email).excludes(id),
    suffix = "Dto"
)]
#[derive(Serialize, Deserialize)]
struct User {
    #[serde(rename = "user_id")]
    id: u64,
    #[serde(rename = "username")]  
    name: String,
    email: String,
}

// Serialization works as expected:
let create_dto = CreateDto {
    name: "Alice".to_string(),
    email: Some("alice@example.com".to_string()),
};

let json = serde_json::to_string(&create_dto)?;
// {"username": "Alice", "email": "alice@example.com"}
```

### Trait Derivation

All derives are inherited by generated variants:

```rust
#[variants(Create: requires(name), Update: requires(id, name))]
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
}

// Both CreateUser and UpdateUser automatically have:
// Debug, Clone, PartialEq, Hash, Serialize, Deserialize
```

## Context-Level Attribute Configuration

### Global Attribute Sets

Apply attributes to all optional/required fields across variants:

```rust
#[variants(
    Create: requires(name, email).excludes(id, admin),
    Update: requires(id, name).optional(email).excludes(admin),
    // Context-level attributes
    optional_attrs = [serde(skip_serializing_if = "Option::is_none"), serde(default)],
    required_attrs = [serde(deny_unknown_fields = false)],
    suffix = "Entity"
)]
#[derive(Debug, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub admin: bool,
}
```

These attributes are applied in addition to field-specific `when_*` attributes and greatly reduce boilerplate.

## Field Groups

Group related fields together for easier reference:

```rust
#[variants(
    prefix = "UserRequest",
    groups = (
        auth(user_id, token), 
        contact(name, email)
    ),
    Login: requires(auth).default(exclude),
    Register: requires(contact).optional(auth).default(exclude),
    Update: requires(auth, name).default(exclude)
)]
struct UserRequest {
    user_id: String,
    token: String,
    name: String,
    email: String,
    metadata: Option<String>,
}
```

## Naming Configuration

### Prefix and Suffix

```rust
#[variants(
    Create: requires(name),
    Update: requires(id, name),
    prefix = "User",      // Optional prefix
    suffix = "Request"    // Optional suffix
)]
struct Data {
    id: u64,
    name: String,
}
```

Generates:
- `UserCreateRequest`
- `UserUpdateRequest`

If no prefix/suffix specified, uses base struct name:
- `DataCreate`
- `DataUpdate`

## Customizing variant names

The `context_variants` macro accepts two optional arguments: `prefix` and
`suffix`. These strings are prepended and/or appended to the variant name when
constructing the generated struct’s name. For example, with
`prefix = "", suffix = "Request"` and a variant `Get` the generated type
will be named `GetRequest`. Both arguments default to empty strings.


## Derived Traits and Attributes

All non-`variants` attributes applied to the original struct and its fields (such as `#[derive(...)]`, documentation comments, or serde annotations) are propagated to the generated variants. This allows you to derive traits like `Clone`, `Debug`, `Serialize`, or `Deserialize` on your source struct and have those derives automatically apply to every generated variant.

## Generics and Lifetimes

Generics and lifetime parameters declared on the source struct are forwarded to every generated variant along with the original `where` clause. This ensures the variants have the same type constraints as the source.

## Best Practices

### 1. Use Descriptive Variant Names

```rust
// ✅ Good - clear purpose
#[variants(
    CreateUserRequest: requires(name, email),
    UpdateUserRequest: requires(id).optional(name, email),
    UserResponse: requires(id, name, email, created_at)
)]

// ❌ Unclear purpose
#[variants(A: requires(name), B: requires(id))]
```

### 2. Group Related Operations

```rust
// ✅ Good - logical grouping
#[variants(
    Create: requires(name, email).excludes(id, created_at),
    Update: requires(id).optional(name, email).excludes(created_at), 
    Read: requires(id).optional(name, email, created_at),
    suffix = "User"
)]
```

### 3. Use Context-Level Attributes for Common Patterns

```rust
// ✅ Reduces boilerplate
#[variants(
    Create: requires(name).optional(metadata),
    Update: requires(id).optional(name, metadata),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none")],
)]
```

### 4. Leverage all_fields() for Large Structs

```rust
// ✅ Concise for many fields
#[variants(
    Create: requires(name, email).excludes(id, timestamps).default(optional),
    Update: requires(id).optional(all_fields().except(id, timestamps)).default(exclude)
)]
```

## Limitations

* Only supports structs with named fields (no tuple or unit structs)
* Variant names must be valid Rust identifiers
* Fields that are already `Option<T>` remain `Option<T>` in optional variants
* All referenced field names must exist in the base struct

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.