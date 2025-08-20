# Context Variants Specification

## Overview

Context Variants is a Rust procedural macro that generates variant structs from a base struct definition, allowing you to create specialized versions of data structures for different contexts (e.g., Create, Update, Read operations). Each variant can specify which fields are required, optional, or excluded, along with context-specific attributes.

## Basic Syntax

```rust
#[variants(
    VariantName: requires(field1, field2).optional(field3).excludes(field4),
    AnotherVariant: requires(field1).default(exclude),
    // Configuration options
    prefix = "MyPrefix",
    suffix = "MySuffix",
    default = exclude | optional | required
)]
#[derive(Debug, Clone)]
struct MyStruct {
    field1: String,
    field2: u32,
    field3: Option<String>,
    field4: String,
}
```

## Fluent API

### Method Chaining

Each variant uses a fluent API with method chaining:

```rust
#[variants(
    Create: requires(name, email).optional(metadata).excludes(id, password),
    Update: requires(id, name).optional(email, metadata).excludes(password),
    Read: requires(id).optional(name, email, metadata).excludes(password)
)]
```

### Available Methods

- `requires(field1, field2, ...)` - Fields that must be present and non-optional
- `optional(field1, field2, ...)` - Fields that become `Option<T>` 
- `excludes(field1, field2, ...)` - Fields that are completely omitted from the variant
- `default(behavior)` - Sets default behavior for unspecified fields

### Default Behaviors

- `default(exclude)` - Unspecified fields are excluded
- `default(optional)` - Unspecified fields become optional  
- `default(required)` - Unspecified fields remain required

## Advanced Field Selection

### all_fields() Function

Use `all_fields().except(...)` for bulk operations:

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
}
```

This generates:
- `CreateUser`: requires name, email; optional metadata; excludes id, admin, password
- `UpdateUser`: requires id; optional name, email, metadata; excludes password, admin
- `ReadUser`: requires id; optional name, email, admin, metadata; excludes password

## Field-Level Conditional Attributes

### when_* Attributes

Apply different attributes based on field context:

```rust
#[variants(
    Login: requires(email, password).default(exclude),
    Profile: requires(username).optional(email).excludes(password)
)]
#[derive(Debug, Serialize, Deserialize)]
struct User {
    #[when_base(doc = "Email field with base documentation")]
    #[when_optional(serde(skip_serializing_if = "Option::is_none"))]
    #[when_required(serde(rename = "email_address"))]
    pub email: String,
    
    #[when_base(serde(rename = "base_password"))]
    #[when_required(serde(rename = "pwd"))]
    pub password: String,
    
    pub username: String,
}
```

### Attribute Application Rules

- `#[when_base]` - Applied to fields in the base struct only
- `#[when_optional]` - Applied when field is optional in a variant (`Option<T>`)
- `#[when_required]` - Applied when field is required in a variant (non-optional)

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

### Attribute Assignment Syntax

Context-level attributes use the `=` assignment syntax:
- `optional_attrs = [attr1, attr2, ...]` - Applied to all optional fields
- `required_attrs = [attr1, attr2, ...]` - Applied to all required fields

These attributes are applied in addition to field-specific `when_*` attributes.

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

## Variant Type Specifications

You can specify different types for fields in variants using the `as` syntax:

```rust
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
struct ApiEvent {
    pub user_id: u64,
    pub action: String,
    pub response: String,
    pub metadata: String,
    pub timestamp: String,
    pub internal_data: Vec<u8>,
}
```

This generates variants where:
- `Request.user_id` is `String` instead of `u64`
- `Request.response` is `Result<String, ApiError>` instead of `String`
- `Request.metadata` is `Option<serde_json::Value>` instead of `Option<String>`
- `Event.timestamp` is `std::time::SystemTime` instead of `String`

## Base Struct Configuration

### optional_base

Control whether the base struct fields become optional:

```rust
// Makes all base struct fields Option<T>
#[variants(
    CreateRequest: requires(name, email).excludes(id),
    UpdateRequest: requires(id).optional(name, email),
    optional_base = true
)]
struct User {
    pub id: u64,      // Becomes Option<u64> in base struct
    pub name: String, // Becomes Option<String> in base struct
    pub email: String,// Becomes Option<String> in base struct
}

// Keeps base struct fields as-is (default behavior)
#[variants(
    Create: requires(title, content).excludes(id),
    Update: requires(id).optional(title, content),
    optional_base = false  // explicit, same as omitting this line
)]
struct Post {
    pub id: u64,      // Remains u64
    pub title: String,// Remains String
    pub content: String,// Remains String
}
```

When `optional_base = true`:
- All non-`Option<T>` fields in the base struct become `Option<T>`
- Fields that are already `Option<T>` remain `Option<T>`
- Variant structs are unaffected and follow their normal field specifications

### build_base

Control whether the base struct is generated:

```rust
// Only generate variant structs, not the base struct
#[variants(
    CreateRequest: requires(name, email).excludes(id),
    UpdateRequest: requires(id).optional(name, email),
    ReadResponse: requires(id, name, email),
    build_base = false
)]
struct User {  // This struct definition is not generated
    pub id: u64,
    pub name: String,
    pub email: String,
}

// Generate both base struct and variants (default behavior)
#[variants(
    CreateRequest: requires(name, email).excludes(id),
    UpdateRequest: requires(id).optional(name, email),
    build_base = true  // explicit, same as omitting this line
)]
struct User {  // This struct is generated and available for use
    pub id: u64,
    pub name: String,
    pub email: String,
}
```

When `build_base = false`:
- Only the variant structs are generated
- The base struct definition serves only as a template
- You cannot instantiate or use the base struct type

## Generated Code Structure

### Base Struct Preservation

The original struct remains unchanged and can be used normally:

```rust
let user = User {
    id: 1,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    password: "secret".to_string(),
};
```

### Variant Structs

Generated variant structs follow field specifications:

```rust
// From requires(name, email).excludes(id, password).default(optional)
let create_user = CreateUser {
    name: "Alice".to_string(),          // Required
    email: "alice@example.com".to_string(), // Required
    metadata: Some(json!({...})),       // Optional (from default)
    // id and password fields don't exist
};

// From requires(id).optional(name, email).excludes(password) 
let update_user = UpdateUser {
    id: 1,                              // Required
    name: Some("Alice Updated".to_string()), // Optional
    email: None,                        // Optional
    metadata: None,                     // Optional (from default)
    // password field doesn't exist
};
```

## Validation Rules

### Compile-Time Checks

The macro enforces several validation rules:

1. **No field conflicts**: A field cannot be specified in multiple categories for the same variant
   ```rust
   // ❌ ERROR: field 'name' specified multiple times
   Create: requires(name).optional(name)
   ```

2. **Complete coverage**: All fields must be accounted for unless a default is specified
   ```rust
   // ❌ ERROR: field 'email' not specified and no default behavior
   #[variants(Create: requires(name))]
   struct User { name: String, email: String }
   
   // ✅ OK: default behavior specified  
   #[variants(Create: requires(name).default(exclude))]
   struct User { name: String, email: String }
   ```

3. **Valid field names**: Referenced fields must exist in the base struct

## Integration with Serde

### Serialization/Deserialization

All generated variants work seamlessly with serde:

```rust
#[variants(
    Create: requires(name).optional(email).excludes(id),
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

// Usage
let create_dto = CreateDto {
    name: "Alice".to_string(),
    email: Some("alice@example.com".to_string()),
};

let json = serde_json::to_string(&create_dto)?;
// {"username": "Alice", "email": "alice@example.com"}
```

### Attribute Inheritance

- Base struct attributes are inherited by variants
- `when_*` attributes override or supplement base attributes
- Context-level `optional_attrs`/`required_attrs` apply globally

## Error Handling

### Compilation Errors

The macro provides clear error messages for common issues:

```text
error: field 'nonexistent' not found in struct 'User'
   --> src/lib.rs:5:25
    |
5   |     Create: requires(nonexistent),
    |                     ^^^^^^^^^^^^

error: field 'name' specified multiple times in variant 'Create'
   --> src/lib.rs:6:35  
    |
6   |     Create: requires(name).optional(name),
    |                                     ^^^^

error: field 'email' not covered by any specification and no default behavior set
   --> src/lib.rs:3:1
    |
3   | #[variants(Create: requires(name))]
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

## Best Practices

### 1. Use Descriptive Variant Names

```rust
// ✅ Good
#[variants(
    CreateRequest: requires(name, email),
    UpdateRequest: requires(id).optional(name, email),
    ReadResponse: requires(id, name, email, created_at)
)]

// ❌ Avoid generic names
#[variants(
    A: requires(name),
    B: requires(id),
    C: requires(id, name)
)]
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
// ✅ Good - DRY principle
#[variants(
    Create: requires(name).optional(metadata),
    Update: requires(id).optional(name, metadata),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none"), serde(default)],
    required_attrs = [serde(deny_unknown_fields = false)]
)]
```

### 4. Leverage all_fields() for Large Structs

```rust
// ✅ Good - concise for many fields
#[variants(
    Create: requires(name, email).excludes(id, timestamps).default(optional),
    Update: requires(id).optional(all_fields().except(id, timestamps)).default(exclude),
    Read: optional(all_fields()).default(exclude)
)]
```

### 5. Use when_* Attributes for Context-Specific Validation

```rust
#[variants(
    Create: requires(email, password),
    Profile: requires(email).excludes(password)
)]
struct User {
    #[when_required(validate(email))]
    #[when_optional(serde(skip_serializing_if = "Option::is_none"))]
    email: String,
    
    #[when_required(validate(length(min = 8)))]
    password: String,
}
```

## Examples

### REST API DTOs

```rust
#[variants(
    CreateUserRequest: requires(name, email, password).excludes(id, created_at, updated_at),
    UpdateUserRequest: requires(id).optional(name, email).excludes(password, created_at, updated_at),
    UserResponse: requires(id, name, email, created_at).optional(updated_at).excludes(password),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none")],
    required_attrs = [serde(deny_unknown_fields = false)]
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    #[serde(rename = "user_id")]
    pub id: u64,
    
    #[serde(rename = "username")]
    pub name: String,
    
    #[when_required(validate(email))]
    #[serde(rename = "email_address")]
    pub email: String,
    
    #[when_required(validate(length(min = 8)))]
    pub password: String,
    
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

### Database Operations

```rust
#[variants(
    InsertUser: requires(name, email, password_hash).excludes(id, created_at, updated_at),
    UpdateUser: requires(id).optional(name, email, password_hash, updated_at).excludes(created_at),
    SelectUser: requires(id).optional(name, email, created_at, updated_at).excludes(password_hash),
    suffix = "Model"
)]
#[derive(Debug, Clone)]
struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
```

## Migration Guide

### From Previous Versions

If migrating from older syntax, note these changes:

#### Context-Level Attributes

**Old syntax** (separate attribute blocks):
```rust
#[variants(
    Create: requires(name),
    #[optional_attrs]
    serde(skip_serializing_if = "Option::is_none"),
    serde(default)
)]
```

**New syntax** (assignment with arrays):
```rust
#[variants(
    Create: requires(name),
    optional_attrs = [serde(skip_serializing_if = "Option::is_none"), serde(default)]
)]
```

#### Field References

**Old syntax** (implicit field discovery):
```rust
Create: requires(name).default(exclude_rest)
```

**New syntax** (explicit all_fields()):
```rust
Create: requires(name).default(exclude)
// or
Create: requires(name).optional(all_fields().except(name)).default(exclude)
```

## Current Implementation Status

### ✅ Implemented Features

- Fluent API with method chaining
- `requires()`, `optional()`, `excludes()` methods
- `default()` behavior specification
- `all_fields().except()` syntax
- `when_base`, `when_optional`, `when_required` conditional attributes
- Context-level `optional_attrs = [...]` and `required_attrs = [...]`
- Field groups: `groups = (auth(user_id, token), contact(name, email))`
- Prefix and suffix configuration
- Variant type specifications: `field as Type` syntax
- Base struct configuration: `optional_base = true/false`
- Base struct generation control: `build_base = true/false`
- Comprehensive compile-time validation


## License

This specification is part of the Context Variants crate, available under MIT or Apache 2.0 licenses.
