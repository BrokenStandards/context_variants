# context_variants

`context_variants` is a procedural macro for Rust that helps you maintain a single
source of truth for request data models while still providing strongly typed
variants for different contexts (e.g. HTTP methods) at compile time. Instead of
creating multiple request structs that differ only by which fields are required
or optional, you can annotate one struct with `#[context_variants(...)]` and
generate per‑context structs automatically.

## Usage

First, add `context_variants` to your dependencies:

```toml
[dependencies]
context_variants = { path = "path/to/context_variants" }
```

Then annotate your struct:

```rust
use context_variants::context_variants;

#[context_variants(Get, Post, Delete, Put, prefix = "", suffix = "Request")]
struct Request {
    /// User name is required for `Get` and `Post` requests.
    #[ctx_required(Get, Post)]
    pub name: String,

    /// Identifier is required for `Get`, `Delete` and `Put` requests.
    #[ctx_required(Get, Delete, Put)]
    pub id: u64,

    /// A free form note. If not marked, fields default to optional for all variants.
    pub note: String,
    
    /// This field only appears in Post requests, completely excluded from others.
    #[ctx_never(Get, Delete, Put)]
    pub creation_data: String,
}
```

This macro generates four structs: `GetRequest`, `PostRequest`, `DeleteRequest`
and `PutRequest`. For each generated struct:

* Fields listed under `#[ctx_required(...)]` will have their original type in those
  variants.
* Any field not listed for a particular variant will be wrapped in
  `Option<...>`, unless its type is already an `Option`, in which case it is
  preserved.
* You can override default behavior using `#[ctx_optional(...)]` to explicitly make
  fields optional for specific variants, `#[ctx_never(...)]` to completely exclude
  fields from specific variants, `#[ctx_always_required]` to force a field
  to be non‑optional across all variants, or `#[ctx_always_optional]` to force a
  field to be optional in every variant.

### Derived traits and attributes

All non‑`context_variants` attributes applied to the original struct and its
fields (such as `#[derive(...)]`, documentation comments, or serde
annotations) are propagated to the generated variants. This allows you to
derive traits like `Clone`, `Debug`, `Serialize`, or `Deserialize` on your
source struct and have those derives automatically apply to every generated
variant.

### Generics and lifetimes

Generics and lifetime parameters declared on the source struct are forwarded to
every generated variant along with the original `where` clause. This ensures
the variants have the same type constraints as the source.

### Improved attribute handling

The macro provides several ways to improve the developer experience when working with attributes:

#### Default attributes for optional and required fields

You can specify default attributes that should be automatically applied to all optional or required fields:

```rust
#[context_variants(Create, Update, suffix = "Request")]
#[ctx_default_optional_attrs(serde(skip_serializing_if = "Option::is_none"))]
#[ctx_default_required_attrs(serde(deny_unknown_fields = false))]
#[derive(Serialize, Deserialize)]
struct UserRequest {
    #[ctx_required(Update)]
    pub id: u64, // Gets deny_unknown_fields = false when required
    
    pub name: String, // Gets skip_serializing_if when optional, deny_unknown_fields when required
}
```

This eliminates the need to manually add `skip_serializing_if` to every optional field.

#### Field-specific attribute overrides

You can still override the default behavior for specific fields:

```rust
#[ctx_optional_attr(serde(deserialize_with = "custom_optional_deserializer"))]
#[ctx_required_attr(serde(deserialize_with = "custom_required_deserializer"))]
pub email: String, // Uses custom deserializers instead of defaults

#[ctx_no_default_attrs]
pub password: String, // Opts out of all default attributes
```

#### Base struct vs variant struct attributes

Some attributes only make sense on the complete base struct (like database mappings) but not on partial variant structs:

```rust
#[context_variants(Create, Update, suffix = "Request")]
#[ctx_base_only(sqlx, diesel, validator)] // These won't appear on generated variants
#[derive(Debug, Serialize, Deserialize)] // This will appear on variants
#[sqlx(derive(FromRow))] // Only on base struct
#[diesel(table_name = users)] // Only on base struct
struct UserRequest {
    // fields...
}
```

The generated `CreateRequest` and `UpdateRequest` structs will have `Debug`, `Serialize`, and `Deserialize` but NOT the `sqlx` or `diesel` attributes.

## Field attributes

### `#[ctx_required(Variant1, Variant2, ...)]`

Mark a field as required (non‑optional) for the specified variants.

### `#[ctx_optional(Variant1, Variant2, ...)]`

Mark a field as explicitly optional for the specified variants, even if it is
listed in `#[ctx_required(...)]` elsewhere. This is useful to opt out of a
required field for a particular variant.

### `#[ctx_never(Variant1, Variant2, ...)]`

Completely exclude a field from the specified variants. The field will not
appear in those generated structs at all. This is useful for fields that are
only relevant to specific contexts.

```rust
#[ctx_never(Get, Delete)]
pub creation_metadata: String, // Only appears in Post and Put variants
```

### `#[ctx_always_required]`

Force a field to be required in every generated variant. This overrides
anything specified in `#[ctx_required(...)]` or `#[ctx_optional(...)]`.

### `#[ctx_always_optional]`

Force a field to be optional in every generated variant. This supersedes
`#[ctx_required(...)]` and `#[ctx_always_required]`.

### `#[ctx_no_default_attrs]`

Opt out of any default attributes specified at the struct level. This field will only
get explicitly specified `#[ctx_optional_attr(...)]` and `#[ctx_required_attr(...)]`
attributes.

### `#[ctx_optional_attr(...)]`

Specify attributes to apply only when this field appears as optional in a variant.

### `#[ctx_required_attr(...)]`

Specify attributes to apply only when this field appears as required in a variant.

### `#[ctx_base_only_attrs(...)]`

Specify attribute names that should only appear on this field in the base struct,
not in generated variants. Useful for database mappings, validation rules, or other
attributes that only make sense in the context of the complete struct.

```rust
#[ctx_base_only_attrs(sqlx, validator)]
#[sqlx(rename = "user_id")]
#[validator(range(min = 1))]
pub id: u64, // sqlx and validator attrs only on base struct field
```

## Struct-level attributes

### `#[ctx_default_optional_attrs(...)]`

Specify default attributes to automatically apply to all fields when they appear as
optional in variants. This greatly reduces boilerplate for common patterns like
`serde(skip_serializing_if = "Option::is_none")`.

### `#[ctx_default_required_attrs(...)]`

Specify default attributes to automatically apply to all fields when they appear as
required in variants.

### `#[ctx_base_only(...)]`

Specify attribute names that should only appear on the base struct, not on generated
variants. Useful for database mapping attributes that don't make sense on partial structs.

## Comprehensive attribute handling solution

The `context_variants` macro provides a complete solution to attribute handling challenges when generating variant structs. Here's how it addresses common developer pain points:

### Problem: Repetitive attribute boilerplate
**Before**: Manually adding `serde(skip_serializing_if = "Option::is_none")` to every optional field
**After**: Use `#[ctx_default_optional_attrs(...)]` to apply it automatically

### Problem: Database attributes on API structs  
**Before**: Generated API request structs polluted with `#[sqlx(...)]`, `#[diesel(...)]` attributes
**After**: Use `#[ctx_base_only(...)]` to keep them only on the base struct

### Problem: Field validation on partial structs
**Before**: Validation rules that expect complete data failing on partial variant structs
**After**: Use `#[ctx_base_only_attrs(...)]` on individual fields to keep validation only on base

### Problem: Complex state-dependent serialization
**Before**: Difficult to have different serializers for required vs optional versions of the same field
**After**: Use `#[ctx_required_attr(...)]` and `#[ctx_optional_attr(...)]` for field-specific handling

### Complete example

```rust
#[context_variants(Create, Update, Delete, suffix = "Request")]
#[ctx_default_optional_attrs(serde(skip_serializing_if = "Option::is_none"))]
#[ctx_default_required_attrs(serde(deny_unknown_fields = false))]
#[ctx_base_only(sqlx, diesel, validator)] // Struct-level filtering
#[derive(Debug, Serialize, Deserialize)]
#[sqlx(derive(FromRow))] // Only on base
#[diesel(table_name = users)] // Only on base
struct UserRequest {
    #[ctx_required(Update, Delete)]
    #[ctx_base_only_attrs(sqlx)] // Field-level filtering
    #[sqlx(rename = "user_id")] // Only on base field
    pub id: u64,

    #[ctx_required(Create)]
    #[ctx_optional_attr(serde(deserialize_with = "opt_email_deser"))]
    #[ctx_required_attr(serde(deserialize_with = "req_email_deser"))]
    pub email: String,

    // This field gets automatic skip_serializing_if when optional
    // and deny_unknown_fields when required
    pub name: String,
}
```

This generates clean variant structs without database/validation noise while preserving the full-featured base struct for complete operations.

## Customizing variant names

The `context_variants` macro accepts two optional arguments: `prefix` and
`suffix`. These strings are prepended and/or appended to the variant name when
constructing the generated struct’s name. For example, with
`prefix = "", suffix = "Request"` and a variant `Get` the generated type
will be named `GetRequest`. Both arguments default to empty strings.

## Limitations

* The macro only supports structs with **named fields**. Tuple structs and
  unit structs are not supported.
* Variant names in `#[ctx_required(...)]`, `#[ctx_optional(...)]`, and `#[ctx_never(...)]` must match one of the
  variant identifiers specified at the macro invocation. Unknown names result
  in a compile‑time error.
* Fields whose types are already `Option<T>` remain `Option<T>` in optional
  variants; nested `Option<Option<T>>` is not generated.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.