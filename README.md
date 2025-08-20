# context_variants

Stop writing the same struct multiple times for different API endpoints! 

`context_variants` is a Rust procedural macro that generates specialized structs from a single base definition. Instead of manually creating separate `CreateUserRequest`, `UpdateUserRequest`, and `UserResponse` structs that are 80% identical, you define your struct once and let the macro generate context-specific variants.

**The Problem:**
```rust
// ❌ Repetitive, error-prone, hard to maintain
struct CreateUserRequest {
    pub name: String,
    pub email: String,
    // No id, created_at fields
}

struct UpdateUserRequest {
    pub id: u64,
    pub name: Option<String>,
    pub email: Option<String>,  
    // No created_at field
}

struct UserResponse {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    // No password field
}
```

**The Solution:**
```rust
// ✅ Define once, generate many variants
#[variants(
    CreateRequest: requires(name, email).excludes(id, created_at),
    UpdateRequest: requires(id).optional(name, email).excludes(created_at),
    Response: requires(id, name, email, created_at)
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
}
```

## Quick Start

Add `context_variants` to your `Cargo.toml`:

```toml
[dependencies]
context_variants = "0.1.0"
serde = { version = "1.0", features = ["derive"] }
```

Define your base struct and specify variants:

```rust
use context_variants::variants;
use serde::{Serialize, Deserialize};

#[variants(
    CreateRequest: requires(name, email).excludes(id, created_at),
    UpdateRequest: requires(id).optional(name, email).excludes(created_at),
    Response: requires(id, name, email, created_at)
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
}
```

This generates three structs you can use immediately:

```rust
// For creating users
let create_req = CreateRequest {
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    // No id or created_at fields
};

// For updating users  
let update_req = UpdateRequest {
    id: 1,
    name: Some("Alice Smith".to_string()),
    email: None, // Not changing email
    // No created_at field
};

// For API responses
let response = Response {
    id: 1,
    name: "Alice Smith".to_string(), 
    email: "alice@example.com".to_string(),
    created_at: Utc::now(),
};
```

## How It Works

The `variants` macro uses a fluent API to specify what happens to each field in your variants:

- **`requires(field1, field2, ...)`** - Fields that must be present and non-optional
- **`optional(field1, field2, ...)`** - Fields that become `Option<T>`  
- **`excludes(field1, field2, ...)`** - Fields that are completely omitted
- **`default(behavior)`** - What to do with unspecified fields

### Default Behaviors

Set what happens to fields you don't explicitly mention:

- `default(exclude)` - Unspecified fields are omitted
- `default(optional)` - Unspecified fields become `Option<T>`
- `default(required)` - Unspecified fields stay as-is (default behavior)

```rust
#[variants(
    Create: requires(name, email).default(exclude),    // Only name, email
    Update: requires(id).default(optional),            // id + everything else optional
    Read: requires(id).optional(name, email).default(exclude) // id + optional name, email
)]
```

### Your Base Struct Stays Unchanged

The original struct remains available and works exactly as before:

```rust
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    created_at: Utc::now(),
};
```

## Real-World Example: REST API

Here's how you'd use `context_variants` for a typical REST API with proper error handling and serde integration:

```rust
use context_variants::variants;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[variants(
    CreateRequest: requires(username, email, password).excludes(id, created_at, updated_at),
    UpdateRequest: requires(id).optional(username, email).excludes(password, created_at, updated_at),
    PublicProfile: requires(id, username, created_at).optional(updated_at).excludes(password, email),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none")]
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    #[serde(rename = "user_id")]
    pub id: u64,
    
    pub username: String,
    
    #[serde(rename = "email_address")]
    pub email: String,
    
    pub password: String,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Your API handlers become clean and type-safe:
async fn create_user(request: CreateRequest) -> Result<PublicProfile, Error> {
    // request only has: username, email, password
    let user = User {
        id: generate_id(),
        username: request.username,
        email: request.email,
        password: hash_password(request.password),
        created_at: Utc::now(),
        updated_at: None,
    };
    
    save_user(&user).await?;
    
    // Return public profile (no password/email)
    Ok(PublicProfile {
        id: user.id,
        username: user.username,
        created_at: user.created_at,
        updated_at: user.updated_at,
    })
}

async fn update_user(request: UpdateRequest) -> Result<PublicProfile, Error> {
    // request has: id (required), username/email (optional)
    let mut user = find_user(request.id).await?;
    
    if let Some(username) = request.username {
        user.username = username;
    }
    if let Some(email) = request.email {
        user.email = email;
    }
    user.updated_at = Some(Utc::now());
    
    save_user(&user).await?;
    Ok(PublicProfile::from(user))
}
```

## Configuration Options

### Naming Your Variants

Control the generated struct names with `prefix` and `suffix`:

```rust
#[variants(
    Create: requires(name),
    Update: requires(id, name),
    prefix = "User",      // Optional
    suffix = "Request"    // Optional  
)]
struct Data {
    id: u64,
    name: String,
}
```

Generates: `UserCreateRequest`, `UserUpdateRequest`

Without prefix/suffix, uses the base struct name: `DataCreate`, `DataUpdate`

## Advanced Features

### Bulk Field Operations

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
    last_login: Option<DateTime<Utc>>,
}
```

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

### Variant Type Specifications

Change field types in variants using the `as` syntax:

```rust
#[variants(
    ApiRequest: requires(
        user_id as String,        // u64 -> String  
        response as Result<String, Error> // String -> Result<...>
    ).optional(
        metadata as serde_json::Value // String -> serde_json::Value
    ),
    Event: requires(
        user_id,                  // unchanged u64
        timestamp as SystemTime   // String -> SystemTime
    )
)]
struct ApiEvent {
    pub user_id: u64,
    pub response: String,
    pub metadata: String,
    pub timestamp: String,
}
```

This generates variants where fields have different types than the base struct, useful for API boundaries or data transformations.

### Base Struct Configuration

#### optional_base

Make all base struct fields optional:

```rust
#[variants(
    Create: requires(name, email),
    Update: requires(id).optional(name, email),
    optional_base = true  // All base fields become Option<T>
)]
struct User {
    pub id: u64,      // Becomes Option<u64> in base struct
    pub name: String, // Becomes Option<String> in base struct  
    pub email: String // Becomes Option<String> in base struct
}
```

#### build_base

Control whether the base struct is generated:

```rust
#[variants(
    CreateDto: requires(name, email),
    UpdateDto: requires(id).optional(name, email),
    build_base = false  // Only generate variants, not base struct
)]
struct User {  // This struct is NOT generated/available
    pub id: u64,
    pub name: String,
    pub email: String,
}
```

## Validation and Error Handling

The macro provides comprehensive compile-time validation:

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

Error messages are clear and point to the exact location of the problem.

## Integration with Existing Code

### Serde Compatibility

All serde attributes are preserved and work correctly. The macro plays nicely with:

- `#[serde(rename = "...")]`
- `#[serde(skip_serializing_if = "...")]`  
- `#[serde(default)]`
- All other serde attributes

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

### Generics and Lifetimes

Generics and lifetime parameters are forwarded to every generated variant with the same constraints:

```rust
#[variants(
    Create: requires(name),
    Update: requires(id, name)
)]
#[derive(Debug)]
struct User<T: Clone> 
where 
    T: Send + Sync,
{
    id: u64,
    name: String,
    data: T,
}

// Generated variants have the same generic constraints
```

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