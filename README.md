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

### Serde compatibility

The macro works seamlessly with serde serialization and deserialization. All
serde attributes (like `#[serde(rename = "...")]`, `#[serde(skip_serializing_if = "...")]`,
etc.) applied to fields are preserved in the generated variants. This allows you
to have consistent JSON/serialization schemas across all your variant structs.

```rust
#[context_variants(Create, Update, suffix = "Request")]
#[derive(Serialize, Deserialize)]
struct UserRequest {
    #[ctx_required(Update)]
    #[serde(rename = "user_id")]
    pub id: u64,
    
    #[ctx_never(Update)]  // Only in CreateRequest
    #[serde(rename = "password")]
    pub password: String,
}
```

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