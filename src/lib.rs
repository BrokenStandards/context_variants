//! A procedural macro to generate context‐specific struct variants using a fluent API.
//!
//! This crate provides the `variants` attribute macro which uses a modern fluent syntax
//! to define struct variants with different field requirements for different contexts.
//!
//! ## Basic Usage
//!
//! ```rust
//! use context_variants::variants;
//! use serde::{Serialize, Deserialize};
//!
//! #[variants(
//!     Create: requires(name, email).excludes(id),
//!     Update: requires(id).optional(name, email),
//!     Read: requires(id, name, email),
//!     suffix = "Request"
//! )]
//! #[derive(Debug, Serialize, Deserialize)]
//! struct User {
//!     pub id: u64,
//!     pub name: String,
//!     pub email: String,
//! }
//! ```
//!
//! This generates `CreateRequest`, `UpdateRequest`, and `ReadRequest` structs.
//!
//! ## Field-Level Conditional Attributes
//!
//! Use `when_*` attributes to apply different attributes based on field context:
//! - `#[when_base]` - applied only to the base struct
//! - `#[when_optional]` - applied when field is optional (`Option<T>`)
//! - `#[when_required]` - applied when field is required (non-optional)
//!
//! ## Global Attribute Configuration
//!
//! Apply attributes to all optional/required fields across variants:
//! - `optional_attrs = [attr1, attr2, ...]` - applied to all optional fields
//! - `required_attrs = [attr1, attr2, ...]` - applied to all required fields

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{spanned::Spanned, Attribute, DeriveInput, Field, Fields, Meta, Type, Visibility, parse::ParseStream};
use proc_macro_error::{emit_error, proc_macro_error};


/// Field reference within a group definition - can be a field name or all_fields() with exceptions
#[derive(Debug, Clone, PartialEq)]
enum GroupFieldRef {
    /// Regular field name
    Field(Ident),
    /// all_fields() with optional exceptions
    AllFields { except: Vec<Ident> },
}




/// Parsed top-level attribute arguments.
#[derive(Debug, Default)]
struct VariantList {
    variants: Vec<Ident>,
    prefix: Option<String>,
    suffix: Option<String>,

    /// Default attributes to apply to all optional fields
    default_optional_attrs: Vec<Attribute>,
    /// Default attributes to apply to all required fields  
    default_required_attrs: Vec<Attribute>,

    /// NEW: Fluent context definitions (Context: requires(field1, field2))
    fluent_contexts: Vec<FluentContext>,
    /// Global default behavior for unspecified fields
    global_default: Option<DefaultBehavior>,
    /// Named field groups for reuse
    field_groups: std::collections::HashMap<String, Vec<Ident>>,
    /// Temporary storage for group field references that need expansion
    group_field_refs: std::collections::HashMap<String, Vec<GroupFieldRef>>,
    /// Whether to generate the base struct (defaults to true)
    build_base: bool,
    /// Whether to make all fields in the base struct optional (defaults to false)
    optional_base: bool,
}

/// Represents a fluent context definition like "Create: requires(name, email)"
/// Field reference that can be either a regular field name or all_fields() with exceptions
#[derive(Debug, Clone, PartialEq)]
enum FieldRef {
    /// Regular field name
    Field(Ident),
    /// Field with variant type specification (field_name: Type)
    FieldWithType { field: Ident, variant_type: Type },
    /// all_fields() with optional exceptions
    AllFields { except: Vec<Ident> },
    /// Named group reference with exceptions
    GroupWithExcept { group: Ident, except: Vec<Ident> },
}

impl FieldRef {
    /// Check if this field reference matches a given field name
    fn matches_field(&self, field_name: &Ident, all_struct_fields: &[Ident], field_groups: &std::collections::HashMap<String, Vec<Ident>>) -> bool {
        match self {
            FieldRef::Field(name) => name == field_name,
            FieldRef::FieldWithType { field, .. } => field == field_name,
            FieldRef::AllFields { except } => {
                // Match if field is in all_struct_fields but not in exceptions
                all_struct_fields.contains(field_name) && !except.contains(field_name)
            }
            FieldRef::GroupWithExcept { group, except } => {
                // Match if field is in the named group but not in exceptions
                if let Some(group_fields) = field_groups.get(&group.to_string()) {
                    group_fields.contains(field_name) && !except.contains(field_name)
                } else {
                    false
                }
            }
        }
    }

    /// Get the variant type for this field reference, if specified
    fn get_variant_type(&self) -> Option<&Type> {
        match self {
            FieldRef::FieldWithType { variant_type, .. } => Some(variant_type),
            _ => None,
        }
    }

}

#[derive(Debug, Clone)]
struct FluentContext {
    name: Ident,
    required_fields: Vec<FieldRef>,
    optional_fields: Vec<FieldRef>, 
    excluded_fields: Vec<FieldRef>,
    default_behavior: Option<DefaultBehavior>,
    /// Span of the end of the expression (for better error positioning)
    end_span: Span,
}

/// Default behavior for unspecified fields
#[derive(Debug, Clone, PartialEq)]
enum DefaultBehavior {
    Required,
    Optional,
    Exclude,
}

/// Helper to parse fluent context expressions
struct FluentContextParser;

impl FluentContextParser {
    /// Parse fluent expression: requires(name, email).optional(metadata).excludes(password)
    fn parse_fluent_expr(context_name: Ident, expr: &syn::Expr) -> Result<FluentContext, syn::Error> {
        match expr {
            syn::Expr::Call(call) => {
                // Handle "requires(field1, field2)" syntax
                Self::parse_function_call(context_name, call)
            }
            syn::Expr::MethodCall(method_call) => {
                // Handle "requires(field1, field2).optional(field3)" syntax  
                Self::parse_method_chain(context_name, method_call)
            }
            _ => {
                Err(syn::Error::new(expr.span(), "expected function call like 'requires(field1, field2)'"))
            }
        }
    }

    fn parse_function_call(context_name: Ident, call: &syn::ExprCall) -> Result<FluentContext, syn::Error> {
        // Parse "requires(field1, field2)"
        let func_name = match call.func.as_ref() {
            syn::Expr::Path(path) => {
                path.path.get_ident()
                    .ok_or_else(|| syn::Error::new(path.span(), "expected function name"))?
                    .to_string()
            }
            _ => return Err(syn::Error::new(call.func.span(), "expected function name")),
        };
        
        let mut context = FluentContext {
            name: context_name,
            required_fields: Vec::new(),
            optional_fields: Vec::new(),
            excluded_fields: Vec::new(),
            default_behavior: None,
            end_span: call.span(),
        };
        
        let fields = Self::parse_field_list(&call.args)?;
        
        match func_name.as_str() {
            "requires" => context.required_fields = fields,
            "optional" => context.optional_fields = fields,
            "excludes" => context.excluded_fields = fields,
            "default" => {
                // Parse default behavior: default(optional), default(required), default(exclude)
                if fields.len() != 1 {
                    return Err(syn::Error::new(call.span(), "default() expects exactly one argument"));
                }
                let default_str = match &fields[0] {
                    FieldRef::Field(ident) => ident.to_string(),
                    _ => return Err(syn::Error::new(call.func.span(), "expected field name for default behavior")),
                };
                context.default_behavior = Some(match default_str.as_str() {
                    "required" => DefaultBehavior::Required,
                    "optional" => DefaultBehavior::Optional,
                    "exclude" => DefaultBehavior::Exclude,
                    _ => return Err(syn::Error::new(call.func.span(), "expected 'required', 'optional', or 'exclude'")),
                });
            }
            _ => return Err(syn::Error::new(call.func.span(), "expected 'requires', 'optional', 'excludes', or 'default'")),
        }
        
        Ok(context)
    }
    
    fn parse_method_chain(context_name: Ident, method_call: &syn::ExprMethodCall) -> Result<FluentContext, syn::Error> {
        let mut context = FluentContext {
            name: context_name,
            required_fields: Vec::new(),
            optional_fields: Vec::new(),
            excluded_fields: Vec::new(),
            default_behavior: None,
            end_span: method_call.span(),
        };
        
        // Start by parsing the receiver (the initial function call)
        let mut method_calls = Vec::new();
        
        // First, collect all method calls in the chain
        let mut temp_method_call = method_call;
        loop {
            method_calls.push((temp_method_call.method.clone(), &temp_method_call.args));
            
            // Check if the receiver is also a method call
            match &*temp_method_call.receiver {
                syn::Expr::MethodCall(nested_method) => {
                    temp_method_call = nested_method;
                }
                syn::Expr::Call(call) => {
                    // This is the base function call, parse it first
                    let base_context = Self::parse_function_call(context.name.clone(), call)?;
                    context.required_fields = base_context.required_fields;
                    context.optional_fields = base_context.optional_fields;
                    context.excluded_fields = base_context.excluded_fields;
                    break;
                }
                _ => {
                    return Err(syn::Error::new(
                        temp_method_call.receiver.span(),
                        "method chain must start with a function call like 'requires(...)'",
                    ));
                }
            }
        }
        
        // Process method calls in reverse order (since we collected them backwards)
        for (method_name, args) in method_calls.into_iter().rev() {
            let fields = Self::parse_field_list(args)?;
            
            match method_name.to_string().as_str() {
                "requires" => context.required_fields.extend(fields),
                "optional" => context.optional_fields.extend(fields),
                "excludes" => context.excluded_fields.extend(fields),
                "default" => {
                    // Parse default behavior: .default(optional), .default(required), .default(exclude)
                    if fields.len() != 1 {
                        return Err(syn::Error::new(method_name.span(), "default() expects exactly one argument"));
                    }
                    let default_str = match &fields[0] {
                        FieldRef::Field(ident) => ident.to_string(),
                        _ => return Err(syn::Error::new(method_name.span(), "expected field name for default behavior")),
                    };
                    context.default_behavior = Some(match default_str.as_str() {
                        "required" => DefaultBehavior::Required,
                        "optional" => DefaultBehavior::Optional,
                        "exclude" => DefaultBehavior::Exclude,
                        _ => return Err(syn::Error::new(method_name.span(), "expected 'required', 'optional', or 'exclude'")),
                    });
                }
                _ => {
                    return Err(syn::Error::new(
                        method_name.span(),
                        "expected 'requires', 'optional', 'excludes', or 'default'",
                    ));
                }
            }
        }
        
        Ok(context)
    }
    
    fn parse_field_list(args: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>) -> Result<Vec<FieldRef>, syn::Error> {
        let mut fields = Vec::new();
        
        for arg in args {
            match arg {
                syn::Expr::Path(path) => {
                    if let Some(ident) = path.path.get_ident() {
                        fields.push(FieldRef::Field(ident.clone()));
                    } else {
                        return Err(syn::Error::new(path.span(), "expected field name"));
                    }
                }
                syn::Expr::Cast(cast_expr) => {
                    // Handle "field_name as Type" syntax
                    if let syn::Expr::Path(path) = cast_expr.expr.as_ref() {
                        if let Some(field_ident) = path.path.get_ident() {
                            fields.push(FieldRef::FieldWithType {
                                field: field_ident.clone(),
                                variant_type: (*cast_expr.ty).clone(),
                            });
                        } else {
                            return Err(syn::Error::new(path.span(), "expected field name before 'as'"));
                        }
                    } else {
                        return Err(syn::Error::new(cast_expr.expr.span(), "expected field name before 'as'"));
                    }
                }
                syn::Expr::Call(call) => {
                    // Handle all_fields() function call
                    if let syn::Expr::Path(path) = call.func.as_ref() {
                        if let Some(ident) = path.path.get_ident() {
                            if ident == "all_fields" {
                                // This is all_fields() call, parse optional arguments
                                let mut except_fields = Vec::new();
                                for arg in &call.args {
                                    match arg {
                                        syn::Expr::Path(field_path) => {
                                            if let Some(field_ident) = field_path.path.get_ident() {
                                                except_fields.push(field_ident.clone());
                                            } else {
                                                return Err(syn::Error::new(arg.span(), "expected field name in all_fields() arguments"));
                                            }
                                        }
                                        _ => return Err(syn::Error::new(arg.span(), "expected field name in all_fields() arguments")),
                                    }
                                }
                                fields.push(FieldRef::AllFields { except: except_fields });
                                continue;
                            }
                        }
                    }
                    return Err(syn::Error::new(call.span(), "unsupported function call"));
                }
                syn::Expr::MethodCall(method_call) => {
                    // Handle all_fields().except(field1, field2) method call
                    if let syn::Expr::Call(base_call) = method_call.receiver.as_ref() {
                        if let syn::Expr::Path(path) = base_call.func.as_ref() {
                            if let Some(ident) = path.path.get_ident() {
                                if ident == "all_fields" && method_call.method == "except" {
                                    // Parse the except() arguments
                                    let mut except_fields = Vec::new();
                                    for arg in &method_call.args {
                                        match arg {
                                            syn::Expr::Path(field_path) => {
                                                if let Some(field_ident) = field_path.path.get_ident() {
                                                    except_fields.push(field_ident.clone());
                                                } else {
                                                    return Err(syn::Error::new(arg.span(), "expected field name in except() arguments"));
                                                }
                                            }
                                            _ => return Err(syn::Error::new(arg.span(), "expected field name in except() arguments")),
                                        }
                                    }
                                    fields.push(FieldRef::AllFields { except: except_fields });
                                    continue;
                                }
                            }
                        }
                    }
                    
                    // Handle group.except(field1, field2) method call 
                    if let syn::Expr::Path(path) = method_call.receiver.as_ref() {
                        if let Some(group_ident) = path.path.get_ident() {
                            if method_call.method == "except" {
                                // Parse the except() arguments for named group
                                let mut except_fields = Vec::new();
                                for arg in &method_call.args {
                                    match arg {
                                        syn::Expr::Path(field_path) => {
                                            if let Some(field_ident) = field_path.path.get_ident() {
                                                except_fields.push(field_ident.clone());
                                            } else {
                                                return Err(syn::Error::new(arg.span(), "expected field name in except() arguments"));
                                            }
                                        }
                                        _ => return Err(syn::Error::new(arg.span(), "expected field name in except() arguments")),
                                    }
                                }
                                fields.push(FieldRef::GroupWithExcept { group: group_ident.clone(), except: except_fields });
                                continue;
                            }
                        }
                    }
                    
                    return Err(syn::Error::new(method_call.span(), "unsupported method call"));
                }
                _ => return Err(syn::Error::new(arg.span(), "expected field name, all_fields() function, or group name")),
            }
        }
        
        Ok(fields)
    }
}

impl VariantList {
    // This impl block is kept for any utility methods that might be needed
}

/// Struct representing the processed information for each field of the source struct.
#[derive(Debug)]
struct FieldSpec {
    ident: Ident,
    ty: Type,
    vis: Visibility,
    /// Original attributes excluding our macro-specific attributes.
    attrs: Vec<Attribute>,
    required_in: Vec<Ident>,
    optional_in: Vec<Ident>,
    never_in: Vec<Ident>,
    /// Whether the type is already Option<T> (so we avoid wrapping again).
    is_option: bool,
    /// Attributes to apply when field is optional in a context
    optional_attrs: Vec<Attribute>,
    /// Attributes to apply when field is required in a context  
    required_attrs: Vec<Attribute>,
    /// Attributes that only appear on the base struct (from when_base)
    base_attrs: Vec<Attribute>,
    /// Variant-specific types for this field (variant_name -> Type)
    variant_types: std::collections::HashMap<String, Type>,
}

/// Performs the expansion of the macro.
fn expand_context_variants(mut cfg: VariantList, input: DeriveInput) -> Result<TokenStream2, syn::Error> {
    // Validate item is a struct with named fields.
    let struct_name = &input.ident;
    let generics = &input.generics;
    let where_clause = &generics.where_clause;
    let vis = &input.vis;

    let fields = match input.data {
        syn::Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref named) => named.named.iter().collect::<Vec<_>>(),
                _ => {
                    return Err(syn::Error::new(data.fields.span(), "context_variants only supports structs with named fields"));
                }
            }
        }
        _ => {
            return Err(syn::Error::new(input.ident.span(), "context_variants can only be applied to structs"));
        }
    };

    // Collect all field names for all_fields() resolution and validation
    let all_field_names: Vec<Ident> = fields.iter()
        .filter_map(|f| f.ident.as_ref().cloned())
        .collect();

    // Now expand group field references (includes all_fields() resolution)
    expand_group_field_references(&mut cfg, &all_field_names)?;

    // Validate fluent contexts for field conflicts and coverage
    validate_fluent_contexts(&cfg, &all_field_names);

    // For each field, collect rules and remove our macro-specific attributes.
    let mut processed_fields = Vec::new();
    for f in fields {
        processed_fields.push(process_field(f, &cfg, &all_field_names)?);
    }

    // Validate fluent context variant names exist in the variant list.
    for field_spec in &processed_fields {
        for req in &field_spec.required_in {
            if !cfg.variants.iter().any(|v| v == req) {
                return Err(syn::Error::new(req.span(), format!("unknown variant '{}' for required field", req)));
            }
        }
        for opt in &field_spec.optional_in {
            if !cfg.variants.iter().any(|v| v == opt) {
                return Err(syn::Error::new(opt.span(), format!("unknown variant '{}' for optional field", opt)));
            }
        }
        for never in &field_spec.never_in {
            if !cfg.variants.iter().any(|v| v == never) {
                return Err(syn::Error::new(never.span(), format!("unknown variant '{}' for excluded field", never)));
            }
        }
    }

    // Remove macro attributes from original struct attributes (fluent API only)
    let mut struct_attrs = Vec::new();
    
    for attr in input.attrs {
        if is_macro_attr(&attr, "variants") {
            // Skip the main macro attribute
            continue;
        } else {
            struct_attrs.push(attr);
        }
    }

    // Build tokens for original struct but without our field-level macros.
    let orig_fields_tokens = processed_fields.iter().map(|fs| {
        let FieldSpec { ident, ty, vis, attrs, base_attrs, is_option, .. } = fs;
        
        // If optional_base is true, wrap non-Option types in Option<T>
        let field_type = if cfg.optional_base && !is_option {
            quote! { Option<#ty> }
        } else {
            quote! { #ty }
        };
        
        quote! {
            #(#attrs)*
            #(#base_attrs)*
            #vis #ident : #field_type,
        }
    });

    let orig_struct = quote! {
        #(#struct_attrs)*
        #vis struct #struct_name #generics #where_clause {
            #(#orig_fields_tokens)*
        }
    };

    // Generate variant structs.
    let mut variant_tokens = TokenStream2::new();
    let prefix = cfg.prefix.clone().unwrap_or_default();
    let suffix = cfg.suffix.clone().unwrap_or_default();
    for variant in &cfg.variants {
        // Build struct name: prefix + variant + suffix
        let variant_name = format!("{}{}{}", prefix, variant, suffix);
        let variant_ident = Ident::new(&variant_name, variant.span());

        // For each field determine type for this variant
        let var_fields = processed_fields.iter().filter_map(|fs| {
            let FieldSpec { ident, ty, vis, attrs, required_in, optional_in, never_in, is_option, optional_attrs, required_attrs, base_attrs: _, variant_types } = fs;
            
            // Check if this field should be excluded from this variant
            if never_in.iter().any(|v| v == variant) {
                return None; // Skip this field entirely
            }
            
            // Determine if field is required for this variant based on fluent API
            let required_here = if optional_in.iter().any(|v| v == variant) {
                false
            } else if required_in.iter().any(|v| v == variant) {
                true
            } else {
                // Default behavior: fields are optional unless explicitly required
                false
            };
            
            // Check if there's a variant-specific type for this field in this variant
            let field_type = if let Some(variant_type) = variant_types.get(&variant.to_string()) {
                variant_type.clone()
            } else {
                ty.clone()
            };
            
            let ty_tokens: TokenStream2 = if required_here {
                quote! { #field_type }
            } else {
                // If the variant type or original type is Option<...>, preserve it; otherwise wrap in Option
                if is_option_type(&field_type) || *is_option {
                    quote! { #field_type }
                } else {
                    quote! { ::core::option::Option<#field_type> }
                }
            };
            
            // Determine which conditional attributes to apply
            let mut conditional_attrs = if required_here {
                required_attrs.clone()
            } else {
                optional_attrs.clone()
            };
            
            // Add default attributes from global configuration
            if required_here {
                conditional_attrs.extend(cfg.default_required_attrs.iter().cloned());
            } else {
                conditional_attrs.extend(cfg.default_optional_attrs.iter().cloned());
            }
            
            // Filter field attributes for variants
            let variant_field_attrs: Vec<_> = attrs.iter().cloned().collect();
            
            Some(quote! {
                #(#variant_field_attrs)*
                #(#conditional_attrs)*
                #vis #ident : #ty_tokens,
            })
        });

        // Copy generics and where clause
        let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();
        
        // Copy all struct attributes to variants
        // All struct-level attributes should be copied to generated variant structs
        let variant_attrs: Vec<_> = struct_attrs.iter().cloned().collect();
            
        variant_tokens.extend(quote! {
            #(#variant_attrs)*
            #vis struct #variant_ident #impl_generics #where_clause {
                #(#var_fields)*
            }
        });
    }

    // Compose final tokens
    let expanded = if cfg.build_base {
        quote! {
            #orig_struct
            #variant_tokens
        }
    } else {
        quote! {
            #variant_tokens
        }
    };
    Ok(expanded)
}

/// Process a single field, extracting our macro-specific attributes and
/// returning a `FieldSpec` with cleaned attributes.
fn process_field(field: &Field, cfg: &VariantList, all_field_names: &[Ident]) -> Result<FieldSpec, syn::Error> {
    // Ensure field is named.
    let ident = match &field.ident {
        Some(id) => id.clone(),
        None => return Err(syn::Error::new(field.span(), "context_variants does not support tuple structs")),
    };
    let mut required_in: Vec<Ident> = Vec::new();
    let mut optional_in: Vec<Ident> = Vec::new();
    let mut never_in: Vec<Ident> = Vec::new();
    let mut optional_attrs = Vec::new();
    let mut required_attrs = Vec::new();
    let mut base_attrs: Vec<Attribute> = Vec::new();  // Attributes that only appear on base struct
    let mut other_attrs = Vec::new();
    let mut variant_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
    
    // Process field attributes (fluent API only)
    for attr in &field.attrs {
        if is_macro_attr(attr, "when_optional") {
            // Parse the inner attribute and add it to optional_attrs
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            optional_attrs.push(inner_attr);
        } else if is_macro_attr(attr, "when_required") {
            // Parse the inner attribute and add it to required_attrs  
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            required_attrs.push(inner_attr);
        } else if is_macro_attr(attr, "when_base") {
            // Parse the inner attribute and add it to base_attrs (only for base struct)
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            base_attrs.push(inner_attr);
        } else {
            // Keep attribute
            other_attrs.push(attr.clone());
        }
    }
    
    // Process fluent context definitions (new syntax)
    for fluent_ctx in &cfg.fluent_contexts {
        // Check if this field matches any of the required fields
        for field_ref in &fluent_ctx.required_fields {
            if field_ref.matches_field(&ident, all_field_names, &cfg.field_groups) {
                required_in.push(fluent_ctx.name.clone());
                // Store variant type if specified
                if let Some(variant_type) = field_ref.get_variant_type() {
                    variant_types.insert(fluent_ctx.name.to_string(), variant_type.clone());
                }
                break;
            }
        }
        
        // Check if this field matches any of the optional fields
        for field_ref in &fluent_ctx.optional_fields {
            if field_ref.matches_field(&ident, all_field_names, &cfg.field_groups) {
                optional_in.push(fluent_ctx.name.clone());
                // Store variant type if specified
                if let Some(variant_type) = field_ref.get_variant_type() {
                    variant_types.insert(fluent_ctx.name.to_string(), variant_type.clone());
                }
                break;
            }
        }
        
        // Check if this field matches any of the excluded fields
        for field_ref in &fluent_ctx.excluded_fields {
            if field_ref.matches_field(&ident, all_field_names, &cfg.field_groups) {
                never_in.push(fluent_ctx.name.clone());
                // Store variant type if specified (though it won't be used since field is excluded)
                if let Some(variant_type) = field_ref.get_variant_type() {
                    variant_types.insert(fluent_ctx.name.to_string(), variant_type.clone());
                }
                break;
            }
        }
    }
    
    // Apply default behaviors for fields not explicitly specified in fluent contexts
    for fluent_ctx in &cfg.fluent_contexts {
        let field_explicitly_mentioned = fluent_ctx.required_fields.iter().any(|field_ref| field_ref.matches_field(&ident, all_field_names, &cfg.field_groups)) ||
                                         fluent_ctx.optional_fields.iter().any(|field_ref| field_ref.matches_field(&ident, all_field_names, &cfg.field_groups)) ||
                                         fluent_ctx.excluded_fields.iter().any(|field_ref| field_ref.matches_field(&ident, all_field_names, &cfg.field_groups));
        
        if !field_explicitly_mentioned {
            // Apply default behavior for this context
            let default_behavior = fluent_ctx.default_behavior.as_ref()
                .or(cfg.global_default.as_ref())
                .unwrap_or(&DefaultBehavior::Optional); // Ultimate fallback
            
            match default_behavior {
                DefaultBehavior::Required => required_in.push(fluent_ctx.name.clone()),
                DefaultBehavior::Optional => optional_in.push(fluent_ctx.name.clone()),
                DefaultBehavior::Exclude => never_in.push(fluent_ctx.name.clone()),
            }
        }
    }
    
    // Determine if type is Option<...>
    let is_option = is_option_type(&field.ty);
    Ok(FieldSpec {
        ident,
        ty: field.ty.clone(),
        vis: field.vis.clone(),
        attrs: other_attrs,
        required_in,
        optional_in,
        never_in,
        is_option,
        optional_attrs,
        required_attrs,
        base_attrs,
        variant_types,
    })
}

/// Check if an attribute matches our macro attribute name.
fn is_macro_attr(attr: &Attribute, name: &str) -> bool {
    attr.path().is_ident(name)
}

/// Parse when_* attribute to extract the inner attribute.
/// Example: #[when_optional(serde(skip_serializing_if = "Option::is_none"))]
/// Should extract: #[serde(skip_serializing_if = "Option::is_none")]
fn parse_ctx_attr_attribute(attr: &Attribute) -> Result<Attribute, syn::Error> {
    let meta = attr.meta.clone();
    match meta {
        Meta::List(list) => {
            // Parse the inner content as a single Meta
            let inner_meta: Meta = list.parse_args()?;
            
            // Convert Meta back to Attribute
            Ok(Attribute {
                pound_token: syn::Token![#](attr.span()),
                style: syn::AttrStyle::Outer,
                bracket_token: syn::token::Bracket(attr.span()),
                meta: inner_meta,
            })
        }
        _ => {
            Err(syn::Error::new(meta.span(), "expected a list with inner attribute"))
        }
    }
}

/// Determine if the provided type is of the form `Option<...>`. This is used to avoid wrapping
/// `Option` types in another `Option` when generating optional fields.
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        let path = &type_path.path;
        // Check if last segment ident is Option
        if let Some(last) = path.segments.last() {
            if last.ident == "Option" {
                return true;
            }
        }
    }
    false
}

/// New fluent syntax macro for context variants
/// Usage: #[variants(Create: requires(field1), Update: requires(field2), suffix = "Form")]
#[proc_macro_error]
#[proc_macro_attribute]
pub fn variants(args: TokenStream, input: TokenStream) -> TokenStream {
    // Try to parse as mixed fluent/traditional syntax
    let variants_cfg = match parse_mixed_args(args) {
        Ok(cfg) => cfg,
        Err(err) => return err.into_compile_error().into(),
    };

    // Expand field groups in fluent contexts
    //if let Err(err) = expand_group_field_references(&mut variants_cfg,&all_field_names) {
    //    return err.into_compile_error().into();
    //}

    // Parse the annotated item (struct).
    let input_struct = syn::parse_macro_input!(input as syn::DeriveInput);
    let result = match expand_context_variants(variants_cfg, input_struct) {
        Ok(ts) => ts,
        Err(err) => err.into_compile_error(),
    };
    TokenStream::from(result)
}

/// Parse mixed syntax: fluent contexts (Create: requires(name)) and traditional (suffix = "Form")
fn parse_mixed_args(args: TokenStream) -> Result<VariantList, syn::Error> {
    let mut variants = Vec::new();
    let mut fluent_contexts = Vec::new();
    let mut prefix = None;
    let mut suffix = None;
    let mut global_default = None;
    let mut field_groups = std::collections::HashMap::new();
    let mut default_optional_attrs: Vec<Attribute> = Vec::new();
    let mut default_required_attrs: Vec<Attribute> = Vec::new();
    let mut build_base = true;
    let mut optional_base = false;

    // Parse the token stream manually to handle mixed syntax
    let args2: TokenStream2 = args.into();
    let input = syn::parse::Parser::parse2(
        |input: ParseStream| {
            let mut items = Vec::new();
            while !input.is_empty() {
                // Try to parse as "Ident: Expr" or "Ident = Expr" or just "Ident"
                let name: Ident = input.parse()?;
                
                if input.peek(syn::Token![:]) {
                    // This is fluent syntax: "Create: requires(name)"
                    let _: syn::Token![:] = input.parse()?;
                    let expr: syn::Expr = input.parse()?;
                    items.push(MixedArg::FluentContext { name, expr });
                } else if input.peek(syn::Token![=]) {
                    // This is traditional syntax: "suffix = "Form""
                    let _: syn::Token![=] = input.parse()?;
                    let value: syn::Expr = input.parse()?;
                    items.push(MixedArg::NameValue { name, value });
                } else {
                    // This is just a variant name: "Create"
                    items.push(MixedArg::Path { name });
                }
                
                // Parse comma if not at end
                if !input.is_empty() {
                    let _: syn::Token![,] = input.parse()?;
                }
            }
            Ok(items)
        },
        args2,
    )?;
    
    // Process the parsed items
    for item in input {
        match item {
            MixedArg::Path { name } => {
                variants.push(name);
            }
            MixedArg::NameValue { name, value } => {
                let name_str = name.to_string();
                match name_str.as_str() {
                    "prefix" => {
                        let lit_str = match value {
                            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => s.value(),
                            _ => return Err(syn::Error::new(value.span(), "expected string literal")),
                        };
                        prefix = Some(lit_str);
                    }
                    "suffix" => {
                        let lit_str = match value {
                            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => s.value(),
                            _ => return Err(syn::Error::new(value.span(), "expected string literal")),
                        };
                        suffix = Some(lit_str);
                    }
                    "default" => {
                        // Parse global default: default = "optional", default = "required", default = "exclude"
                        let default_str = match &value {
                            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => s.value(),
                            syn::Expr::Path(path) => {
                                // Handle: default = optional (without quotes)
                                if let Some(ident) = path.path.get_ident() {
                                    ident.to_string()
                                } else {
                                    return Err(syn::Error::new(value.span(), "expected identifier or string literal"));
                                }
                            }
                            _ => return Err(syn::Error::new(value.span(), "expected string literal or identifier")),
                        };
                        global_default = Some(match default_str.as_str() {
                            "required" => DefaultBehavior::Required,
                            "optional" => DefaultBehavior::Optional,
                            "exclude" => DefaultBehavior::Exclude,
                            _ => return Err(syn::Error::new(value.span(), "expected 'required', 'optional', or 'exclude'")),
                        });
                    }
                    "groups" => {
                        // Parse groups = auth(user_id, token), contact(name, email)
                        // This uses a simpler syntax that's easier to parse than JSON-like syntax
                        field_groups = parse_groups_expression(&value)?;
                    }
                    "optional_attrs" => {
                        // Parse optional_attrs = [serde(skip_serializing_if = "Option::is_none"), serde(default)]
                        default_optional_attrs = parse_attribute_array(&value)?;
                    }
                    "required_attrs" => {
                        // Parse required_attrs = [serde(deny_unknown_fields = false)]
                        default_required_attrs = parse_attribute_array(&value)?;
                    }
                    "build_base" => {
                        // Parse build_base = true or build_base = false
                        build_base = match &value {
                            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) => b.value,
                            syn::Expr::Path(path) => {
                                // Handle: build_base = true/false (without quotes)
                                if let Some(ident) = path.path.get_ident() {
                                    match ident.to_string().as_str() {
                                        "true" => true,
                                        "false" => false,
                                        _ => return Err(syn::Error::new(value.span(), "expected 'true' or 'false'")),
                                    }
                                } else {
                                    return Err(syn::Error::new(value.span(), "expected boolean literal or identifier"));
                                }
                            }
                            _ => return Err(syn::Error::new(value.span(), "expected boolean literal")),
                        };
                    }
                    "optional_base" => {
                        // Parse optional_base = true or optional_base = false
                        optional_base = match &value {
                            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) => b.value,
                            syn::Expr::Path(path) => {
                                // Handle: optional_base = true/false (without quotes)
                                if let Some(ident) = path.path.get_ident() {
                                    match ident.to_string().as_str() {
                                        "true" => true,
                                        "false" => false,
                                        _ => return Err(syn::Error::new(value.span(), "expected 'true' or 'false'")),
                                    }
                                } else {
                                    return Err(syn::Error::new(value.span(), "expected boolean literal or identifier"));
                                }
                            }
                            _ => return Err(syn::Error::new(value.span(), "expected boolean literal")),
                        };
                    }
                    _ => {
                        return Err(syn::Error::new(name.span(), "unknown parameter"));
                    }
                }
            }
            MixedArg::FluentContext { name, expr } => {
                // Parse the expression as a fluent context
                let fluent_ctx = FluentContextParser::parse_fluent_expr(name.clone(), &expr)?;
                variants.push(name);
                fluent_contexts.push(fluent_ctx);
            }
        }
    }
    
    if variants.is_empty() {
        return Err(syn::Error::new(proc_macro2::Span::call_site(), "no variants specified"));
    }
    
    Ok(VariantList {
        variants,
        prefix,
        suffix,
        default_optional_attrs,
        default_required_attrs,
        fluent_contexts,
        global_default: global_default,
        field_groups: std::collections::HashMap::new(), // Will be populated later after expansion
        group_field_refs: field_groups, // Store the unexpanded group field references
        build_base,
        optional_base,
    })
}

/// Parse groups expression: auth(user_id, token), contact(name, email)
/// Returns a map of group names to GroupFieldRef lists that need to be expanded later
fn parse_groups_expression(expr: &syn::Expr) -> Result<std::collections::HashMap<String, Vec<GroupFieldRef>>, syn::Error> {
    let mut groups = std::collections::HashMap::new();
    
    match expr {
        syn::Expr::Call(call) => {
            // Single group: auth(user_id, token)
            let (group_name, fields) = parse_single_group(call)?;
            groups.insert(group_name, fields);
        }
        syn::Expr::Tuple(tuple) => {
            // Multiple groups: (auth(user_id, token), contact(name, email))
            for elem in &tuple.elems {
                if let syn::Expr::Call(call) = elem {
                    let (group_name, fields) = parse_single_group(call)?;
                    groups.insert(group_name, fields);
                } else {
                    return Err(syn::Error::new(elem.span(), "expected group definition like 'auth(user_id, token)'"));
                }
            }
        }
        _ => {
            return Err(syn::Error::new(expr.span(), "expected group definition like 'auth(user_id, token)' or tuple of groups"));
        }
    }
    
    Ok(groups)
}

/// Expand group field references to concrete field lists
/// This resolves all_fields() and all_fields().except() within group definitions
fn expand_group_field_refs(
    group_field_refs: &[GroupFieldRef], 
    all_struct_fields: &[Ident]
) -> Vec<Ident> {
    let mut result = Vec::new();
    
    for field_ref in group_field_refs {
        match field_ref {
            GroupFieldRef::Field(field_name) => {
                result.push(field_name.clone());
            }
            GroupFieldRef::AllFields { except } => {
                // Add all struct fields except those in the except list
                for struct_field in all_struct_fields {
                    if !except.contains(struct_field) {
                        result.push(struct_field.clone());
                    }
                }
            }
        }
    }
    
    result
}

/// Parse a single group: auth(user_id, token)
fn parse_single_group(call: &syn::ExprCall) -> Result<(String, Vec<GroupFieldRef>), syn::Error> {
    // Get group name
    let group_name = match call.func.as_ref() {
        syn::Expr::Path(path) => {
            path.path.get_ident()
                .ok_or_else(|| syn::Error::new(path.span(), "expected group name"))?
                .to_string()
        }
        _ => return Err(syn::Error::new(call.func.span(), "expected group name")),
    };
    
    // Parse field list - now supports both field names and all_fields() references
    let mut fields = Vec::new();
    for arg in &call.args {
        match arg {
            syn::Expr::Path(path) => {
                if let Some(ident) = path.path.get_ident() {
                    fields.push(GroupFieldRef::Field(ident.clone()));
                } else {
                    return Err(syn::Error::new(arg.span(), "expected field name"));
                }
            }
            syn::Expr::Call(call) => {
                // Handle all_fields() call
                if let syn::Expr::Path(path) = call.func.as_ref() {
                    if let Some(ident) = path.path.get_ident() {
                        if ident == "all_fields" {
                            fields.push(GroupFieldRef::AllFields { except: Vec::new() });
                        } else {
                            return Err(syn::Error::new(arg.span(), "expected field name or all_fields()"));
                        }
                    } else {
                        return Err(syn::Error::new(arg.span(), "expected field name or all_fields()"));
                    }
                } else {
                    return Err(syn::Error::new(arg.span(), "expected field name or all_fields()"));
                }
            }
            syn::Expr::MethodCall(method_call) => {
                // Handle all_fields().except(...) method call
                if let syn::Expr::Call(call) = method_call.receiver.as_ref() {
                    if let syn::Expr::Path(path) = call.func.as_ref() {
                        if let Some(ident) = path.path.get_ident() {
                            if ident == "all_fields" && method_call.method == "except" {
                                // Parse the except arguments
                                let mut except_fields = Vec::new();
                                for except_arg in &method_call.args {
                                    if let syn::Expr::Path(except_path) = except_arg {
                                        if let Some(except_ident) = except_path.path.get_ident() {
                                            except_fields.push(except_ident.clone());
                                        } else {
                                            return Err(syn::Error::new(except_arg.span(), "expected field name in except clause"));
                                        }
                                    } else {
                                        return Err(syn::Error::new(except_arg.span(), "expected field name in except clause"));
                                    }
                                }
                                fields.push(GroupFieldRef::AllFields { except: except_fields });
                            } else {
                                return Err(syn::Error::new(arg.span(), "expected all_fields().except(...)"));
                            }
                        } else {
                            return Err(syn::Error::new(arg.span(), "expected all_fields().except(...)"));
                        }
                    } else {
                        return Err(syn::Error::new(arg.span(), "expected all_fields().except(...)"));
                    }
                } else {
                    return Err(syn::Error::new(arg.span(), "expected field name, all_fields(), or all_fields().except(...)"));
                }
            }
            _ => return Err(syn::Error::new(arg.span(), "expected field name, all_fields(), or all_fields().except(...)")),
        }
    }
    
    Ok((group_name, fields))
}

#[derive(Debug)]
enum MixedArg {
    Path { name: Ident },
    NameValue { name: Ident, value: syn::Expr },
    FluentContext { name: Ident, expr: syn::Expr },
}


/// Expand group field references now that we have access to struct fields
fn expand_group_field_references(
    variants_cfg: &mut VariantList, 
    all_struct_fields: &[Ident]
) -> Result<(), syn::Error> {
    // First, expand group_field_refs to concrete field lists
    for (group_name, field_refs) in &variants_cfg.group_field_refs {
        let expanded_fields = expand_group_field_refs(field_refs, all_struct_fields);
        variants_cfg.field_groups.insert(group_name.clone(), expanded_fields);
    }
    
    // Now expand group references in fluent contexts
    for fluent_ctx in &mut variants_cfg.fluent_contexts {
        // Expand required_fields
        let mut expanded_required = Vec::new();
        for field_ref in &fluent_ctx.required_fields {
            match field_ref {
                FieldRef::Field(field_ident) => {
                    if let Some(group_fields) = variants_cfg.field_groups.get(&field_ident.to_string()) {
                        // This is a group name, expand it to individual fields
                        for group_field in group_fields {
                            expanded_required.push(FieldRef::Field(group_field.clone()));
                        }
                    } else {
                        // This is a regular field name
                        expanded_required.push(field_ref.clone());
                    }
                }
                FieldRef::FieldWithType { .. } => {
                    // Field with variant type - keep as-is
                    expanded_required.push(field_ref.clone());
                }
                FieldRef::AllFields { .. } => {
                    // Keep all_fields() as-is (it will be resolved later when we have struct field access)
                    expanded_required.push(field_ref.clone());
                }
                FieldRef::GroupWithExcept { group, except } => {
                    // Expand group to individual fields, excluding specified ones
                    if let Some(group_fields) = variants_cfg.field_groups.get(&group.to_string()) {
                        for group_field in group_fields {
                            if !except.contains(group_field) {
                                expanded_required.push(FieldRef::Field(group_field.clone()));
                            }
                        }
                    } else {
                        return Err(syn::Error::new(group.span(), format!("unknown field group '{}'", group)));
                    }
                }
            }
        }
        fluent_ctx.required_fields = expanded_required;

        // Expand optional_fields
        let mut expanded_optional = Vec::new();
        for field_ref in &fluent_ctx.optional_fields {
            match field_ref {
                FieldRef::Field(field_ident) => {
                    if let Some(group_fields) = variants_cfg.field_groups.get(&field_ident.to_string()) {
                        // This is a group name, expand it to individual fields
                        for group_field in group_fields {
                            expanded_optional.push(FieldRef::Field(group_field.clone()));
                        }
                    } else {
                        // This is a regular field name
                        expanded_optional.push(field_ref.clone());
                    }
                }
                FieldRef::FieldWithType { .. } => {
                    // Field with variant type - keep as-is
                    expanded_optional.push(field_ref.clone());
                }
                FieldRef::AllFields { .. } => {
                    // Keep all_fields() as-is
                    expanded_optional.push(field_ref.clone());
                }
                FieldRef::GroupWithExcept { group, except } => {
                    // Expand group to individual fields, excluding specified ones
                    if let Some(group_fields) = variants_cfg.field_groups.get(&group.to_string()) {
                        for group_field in group_fields {
                            if !except.contains(group_field) {
                                expanded_optional.push(FieldRef::Field(group_field.clone()));
                            }
                        }
                    } else {
                        return Err(syn::Error::new(group.span(), format!("unknown field group '{}'", group)));
                    }
                }
            }
        }
        fluent_ctx.optional_fields = expanded_optional;

        // Expand excluded_fields
        let mut expanded_excluded = Vec::new();
        for field_ref in &fluent_ctx.excluded_fields {
            match field_ref {
                FieldRef::Field(field_ident) => {
                    if let Some(group_fields) = variants_cfg.field_groups.get(&field_ident.to_string()) {
                        // This is a group name, expand it to individual fields
                        for group_field in group_fields {
                            expanded_excluded.push(FieldRef::Field(group_field.clone()));
                        }
                    } else {
                        // This is a regular field name
                        expanded_excluded.push(field_ref.clone());
                    }
                }
                FieldRef::FieldWithType { .. } => {
                    // Field with variant type - keep as-is
                    expanded_excluded.push(field_ref.clone());
                }
                FieldRef::AllFields { .. } => {
                    // Keep all_fields() as-is
                    expanded_excluded.push(field_ref.clone());
                }
                FieldRef::GroupWithExcept { group, except } => {
                    // Expand group to individual fields, excluding specified ones
                    if let Some(group_fields) = variants_cfg.field_groups.get(&group.to_string()) {
                        for group_field in group_fields {
                            if !except.contains(group_field) {
                                expanded_excluded.push(FieldRef::Field(group_field.clone()));
                            }
                        }
                    } else {
                        return Err(syn::Error::new(group.span(), format!("unknown field group '{}'", group)));
                    }
                }
            }
        }
        fluent_ctx.excluded_fields = expanded_excluded;
    }
    
    Ok(())
}

/// Validate fluent contexts for field conflicts and complete coverage
fn validate_fluent_contexts(cfg: &VariantList, all_field_names: &[Ident]) {
    for fluent_ctx in &cfg.fluent_contexts {
        // Check for field conflicts within each context
        let mut field_mentions = std::collections::HashMap::new();
        
        // Track where each field is mentioned
        for field_ref in &fluent_ctx.required_fields {
            for field_name in all_field_names {
                if field_ref.matches_field(field_name, all_field_names, &cfg.field_groups) {
                    let mentions = field_mentions.entry(field_name.clone()).or_insert_with(Vec::new);
                    mentions.push("required");
                }
            }
        }
        
        for field_ref in &fluent_ctx.optional_fields {
            for field_name in all_field_names {
                if field_ref.matches_field(field_name, all_field_names, &cfg.field_groups) {
                    let mentions = field_mentions.entry(field_name.clone()).or_insert_with(Vec::new);
                    mentions.push("optional");
                }
            }
        }
        
        for field_ref in &fluent_ctx.excluded_fields {
            for field_name in all_field_names {
                if field_ref.matches_field(field_name, all_field_names, &cfg.field_groups) {
                    let mentions = field_mentions.entry(field_name.clone()).or_insert_with(Vec::new);
                    mentions.push("excluded");
                }
            }
        }
        
        // Check for conflicts (field mentioned more than once)
        for (field_name, mentions) in &field_mentions {
            if mentions.len() > 1 {
                emit_error!(
                    fluent_ctx.end_span,
                    "field '{}' mentioned multiple times: {}", field_name, mentions.join(", ");
                    label = "conflicting field specifications here"
                );
            }
        }
        
        // Check for complete coverage (every field is either explicitly mentioned or has a default)
        let has_default = fluent_ctx.default_behavior.is_some() || cfg.global_default.is_some();
        
        if !has_default {
            let unmentioned_fields: Vec<&Ident> = all_field_names.iter()
                .filter(|field_name| !field_mentions.contains_key(field_name))
                .collect();
                
            if !unmentioned_fields.is_empty() {
                let field_list: Vec<String> = unmentioned_fields.iter().map(|f| f.to_string()).collect();
                let suggestion = if field_list.len() == 1 {
                    let field = &field_list[0];
                    format!("add .requires({}), .optional({}), .excludes({}), or .default(optional/required/exclude)", field, field, field)
                } else {
                    format!("add .requires({}), .optional({}), .excludes({}), or .default(optional/required/exclude)", 
                           field_list.join(","), field_list.join(","), field_list.join(","))
                };
                emit_error!(
                    fluent_ctx.end_span,
                    "missing fields: {}", field_list.join(", ");
                    help = suggestion;
                    label = "all fields must be specified here if `default(...)` is not set"
                );
            }
        }
    }
}

/// Parse an array of attributes: [serde(skip_serializing_if = "Option::is_none"), serde(default)]
fn parse_attribute_array(expr: &syn::Expr) -> Result<Vec<Attribute>, syn::Error> {
    match expr {
        syn::Expr::Array(array) => {
            let mut attributes = Vec::new();
            for elem in &array.elems {
                // Each element should be a meta that we can convert to an attribute
                let meta = match elem {
                    syn::Expr::Call(call) => {
                        // Handle serde(skip_serializing_if = "Option::is_none") syntax
                        syn::Meta::List(syn::MetaList {
                            path: match call.func.as_ref() {
                                syn::Expr::Path(path) => path.path.clone(),
                                _ => return Err(syn::Error::new(call.func.span(), "expected attribute name")),
                            },
                            delimiter: syn::MacroDelimiter::Paren(Default::default()),
                            tokens: {
                                let args = &call.args;
                                if args.is_empty() {
                                    quote::quote! {}
                                } else {
                                    quote::quote! { #args }
                                }
                            },
                        })
                    }
                    syn::Expr::Path(path) => {
                        // Handle simple attribute names like 'default'
                        syn::Meta::Path(path.path.clone())
                    }
                    _ => return Err(syn::Error::new(elem.span(), "expected attribute")),
                };
                
                // Convert Meta to Attribute
                let attr = Attribute {
                    pound_token: Default::default(),
                    style: syn::AttrStyle::Outer,
                    bracket_token: Default::default(),
                    meta,
                };
                attributes.push(attr);
            }
            Ok(attributes)
        }
        _ => Err(syn::Error::new(expr.span(), "expected array of attributes")),
    }
}

