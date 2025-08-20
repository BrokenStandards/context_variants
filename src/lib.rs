//! A procedural macro to generate context‐specific struct variants.
//!
//! This crate provides an attribute macro, [`context_variants`], which can be
//! attached to a struct definition to generate multiple struct variants with
//! differing required/optional fields. Each field can be marked as required
//! for a subset of the variants via a `#[ctx_required(...)]` attribute, or as
//! explicitly optional via `#[ctx_optional(...)]`. You can also exclude fields
//! entirely from specific variants with `#[ctx_never(...)]`. 
//!
//! Default behavior for each context can be controlled with struct-level attributes:
//! `#[ctx_default_required(...)]`, `#[ctx_default_optional(...)]`, and 
//! `#[ctx_default_never(...)]`. Field-level attributes override these defaults.
//!
//! For improved developer experience with attributes, you can specify default attributes
//! to apply to all optional and required fields:
//! - `#[ctx_default_optional_attrs(...)]` - attributes applied to all optional fields
//! - `#[ctx_default_required_attrs(...)]` - attributes applied to all required fields
//! - `#[ctx_no_default_attrs]` - field-level attribute to opt out of default attributes
//! - `#[ctx_base_only_attrs(...)]` - field-level attribute to specify which attributes should only appear on the base struct field
//!
//! To control which struct-level attributes are copied to generated variants:
//! - `#[ctx_base_only(...)]` - attributes that should only appear on the base struct
//! - `#[ctx_variants_only(...)]` - attributes that should only appear on generated variants
//!
//! See the crate level documentation and the tests for usage examples.

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, Attribute, DeriveInput, Field, Fields, Lit, Meta, Type, Visibility, parse::Parse, parse::ParseStream};
use proc_macro_error::{emit_error, proc_macro_error};

/// The main attribute macro. See crate level docs for details.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn context_variants(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments into variant configuration.
    let args = parse_macro_input!(attr as AttributeArgs);
    let variants_cfg = match VariantList::from_args(args.metas) {
        Ok(cfg) => cfg,
        Err(err) => return err.into_compile_error().into(),
    };

    // Parse the annotated item (struct).
    let input = parse_macro_input!(item as DeriveInput);
    let result = match expand_context_variants(variants_cfg, input) {
        Ok(ts) => ts,
        Err(err) => err.into_compile_error(),
    };
    TokenStream::from(result)
}

/// Custom parser for attribute arguments that can handle both old and new syntax
struct AttributeArgs {
    metas: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>,
}

impl Parse for AttributeArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let metas = input.parse_terminated(Meta::parse, syn::Token![,])?;
        Ok(AttributeArgs { metas })
    }
}

/// Custom parser for fluent context definitions: "Create: requires(name), Update: requires(id)"
#[derive(Debug)]
struct FluentContextArgs {
    contexts: syn::punctuated::Punctuated<FluentContextDef, syn::Token![,]>,
}

#[derive(Debug)]
struct FluentContextDef {
    name: Ident,
    expr: syn::Expr,
}

impl Parse for FluentContextArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let contexts = input.parse_terminated(FluentContextDef::parse, syn::Token![,])?;
        Ok(FluentContextArgs { contexts })
    }
}

impl Parse for FluentContextDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        
        // Check if the next token is a colon
        if input.peek(syn::Token![:]) {
            let _: syn::Token![:] = input.parse()?;
            let expr: syn::Expr = input.parse()?;
            Ok(FluentContextDef { name, expr })
        } else {
            // This is not a fluent context def, it's a regular identifier
            // We need to backtrack - create a dummy expr for now
            // This will be handled in the parsing logic
            Err(syn::Error::new(name.span(), "expected ':' after context name"))
        }
    }
}

/// Field groups definition: groups = { group_name: [field1, field2] }
#[derive(Debug)]
struct FieldGroupsArgs {
    groups: syn::punctuated::Punctuated<FieldGroupDef, syn::Token![,]>,
}

#[derive(Debug)]
struct FieldGroupDef {
    name: Ident,
    fields: Vec<Ident>,
}

impl Parse for FieldGroupsArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let groups = input.parse_terminated(FieldGroupDef::parse, syn::Token![,])?;
        Ok(FieldGroupsArgs { groups })
    }
}

impl Parse for FieldGroupDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let _: syn::Token![:] = input.parse()?;
        
        let content;
        let _bracket = syn::bracketed!(content in input);
        let fields = content.parse_terminated(Ident::parse, syn::Token![,])?
            .into_iter()
            .collect();
            
        Ok(FieldGroupDef { name, fields })
    }
}

/// Parsed top-level attribute arguments.
#[derive(Debug, Default)]
struct VariantList {
    variants: Vec<Ident>,
    prefix: Option<String>,
    suffix: Option<String>,
    default_required: Vec<Ident>,
    default_optional: Vec<Ident>,
    default_never: Vec<Ident>,
    /// Default attributes to apply to all optional fields
    default_optional_attrs: Vec<Attribute>,
    /// Default attributes to apply to all required fields  
    default_required_attrs: Vec<Attribute>,
    /// Attributes that should only appear on the base struct
    base_only_attrs: Vec<String>,
    /// Attributes that should only appear on generated variants
    variants_only_attrs: Vec<String>,
    /// NEW: Fluent context definitions (Context: requires(field1, field2))
    fluent_contexts: Vec<FluentContext>,
    /// Global default behavior for unspecified fields
    global_default: Option<DefaultBehavior>,
    /// Named field groups for reuse
    field_groups: std::collections::HashMap<String, Vec<Ident>>,
}

/// Represents a fluent context definition like "Create: requires(name, email)"
/// Field reference that can be either a regular field name or all_fields() with exceptions
#[derive(Debug, Clone, PartialEq)]
enum FieldRef {
    /// Regular field name
    Field(Ident),
    /// all_fields() with optional exceptions
    AllFields { except: Vec<Ident> },
}

impl FieldRef {
    /// Check if this field reference matches a given field name
    fn matches_field(&self, field_name: &Ident, all_struct_fields: &[Ident]) -> bool {
        match self {
            FieldRef::Field(name) => name == field_name,
            FieldRef::AllFields { except } => {
                // Match if field is in all_struct_fields but not in exceptions
                all_struct_fields.contains(field_name) && !except.contains(field_name)
            }
        }
    }
    
    /// Convert to string for debugging/error messages
    fn to_string(&self) -> String {
        match self {
            FieldRef::Field(name) => name.to_string(),
            FieldRef::AllFields { except } => {
                if except.is_empty() {
                    "all_fields()".to_string()
                } else {
                    let except_names: Vec<String> = except.iter().map(|i| i.to_string()).collect();
                    format!("all_fields().except({})", except_names.join(", "))
                }
            }
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

    /// Parse "Create(requires(field1, field2).optional(field3).excludes(field4))"
    fn parse_context_expr(meta: &Meta) -> Result<FluentContext, syn::Error> {
        match meta {
            Meta::List(list) => {
                // Handle "Create(requires(...))" syntax
                let context_name = list.path.get_ident()
                    .ok_or_else(|| syn::Error::new(list.path.span(), "expected context name"))?
                    .clone();
                
                // Parse the content inside the parentheses
                let content: syn::Expr = list.parse_args()?;
                
                match content {
                    syn::Expr::Call(call) => {
                        // Handle "requires(field1, field2)" syntax
                        Self::parse_function_call(context_name, &call)
                    }
                    syn::Expr::MethodCall(method_call) => {
                        // Handle "requires(field1, field2).optional(field3)" syntax  
                        Self::parse_method_chain(context_name, &method_call)
                    }
                    _ => {
                        Err(syn::Error::new(content.span(), "expected function call like 'requires(field1, field2)'"))
                    }
                }
            }
            Meta::NameValue(nv) => {
                let context_name = nv.path.get_ident()
                    .ok_or_else(|| syn::Error::new(nv.path.span(), "expected context name"))?
                    .clone();
                
                // Parse the value as a function call expression
                match &nv.value {
                    syn::Expr::Call(call) => {
                        // Handle "requires(field1, field2)" syntax
                        Self::parse_function_call(context_name, call)
                    }
                    syn::Expr::MethodCall(method_call) => {
                        // Handle "requires(field1, field2).optional(field3)" syntax  
                        Self::parse_method_chain(context_name, method_call)
                    }
                    _ => {
                        Err(syn::Error::new(nv.value.span(), "expected function call like 'requires(field1, field2)'"))
                    }
                }
            }
            _ => Err(syn::Error::new(meta.span(), "expected fluent context definition")),
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
                    return Err(syn::Error::new(method_call.span(), "unsupported method call"));
                }
                _ => return Err(syn::Error::new(arg.span(), "expected field name or all_fields() function")),
            }
        }
        
        Ok(fields)
    }
}

impl VariantList {
    fn from_args(args: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>) -> Result<Self, syn::Error> {
        let mut variants = Vec::new();
        let mut fluent_contexts = Vec::new();
        let mut prefix = None;
        let mut suffix = None;
        
        for meta in args {
            // Clone the meta for potential fluent context parsing
            let meta_clone = meta.clone();
            
            match meta {
                Meta::Path(path) => {
                    if let Some(ident) = path.get_ident() {
                        variants.push(ident.clone());
                    } else {
                        return Err(syn::Error::new(path.span(), "expected an identifier for variant name"));
                    }
                }
                Meta::NameValue(nv) => {
                    if let Some(ident) = nv.path.get_ident() {
                        let ident_str = ident.to_string();
                        
                        // Check if this looks like a fluent context definition
                        if ident_str != "prefix" && ident_str != "suffix" {
                            // Try to parse as fluent context: "Create: requires(name, email)"
                            match FluentContextParser::parse_context_expr(&meta_clone) {
                                Ok(fluent_ctx) => {
                                    variants.push(fluent_ctx.name.clone());
                                    fluent_contexts.push(fluent_ctx);
                                    continue;
                                }
                                Err(_) => {
                                    // Fall through to handle as prefix/suffix
                                }
                            }
                        }
                        
                        // Handle prefix/suffix
                        let lit = match nv.value {
                            syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(ref s), .. }) => s.value(),
                            _ => {
                                return Err(syn::Error::new(nv.value.span(), "expected a string literal for prefix/suffix"));
                            }
                        };
                        match ident_str.as_str() {
                            "prefix" => {
                                if prefix.is_some() {
                                    return Err(syn::Error::new(nv.span(), "duplicate prefix definition"));
                                }
                                prefix = Some(lit);
                            }
                            "suffix" => {
                                if suffix.is_some() {
                                    return Err(syn::Error::new(nv.span(), "duplicate suffix definition"));
                                }
                                suffix = Some(lit);
                            }
                            _ => {
                                return Err(syn::Error::new(ident.span(), "unknown argument; expected prefix, suffix, or fluent context definition"));
                            }
                        }
                    } else {
                        return Err(syn::Error::new(nv.path.span(), "invalid name-value syntax"));
                    }
                }
                other => {
                    return Err(syn::Error::new(other.span(), "unexpected attribute argument"));
                }
            }
        }
        
        if variants.is_empty() && fluent_contexts.is_empty() {
            return Err(syn::Error::new(Span::call_site(), "no variants specified"));
        }
        
        Ok(VariantList { 
            variants, 
            prefix, 
            suffix, 
            default_required: Vec::new(),
            default_optional: Vec::new(),
            default_never: Vec::new(),
            default_optional_attrs: Vec::new(),
            default_required_attrs: Vec::new(),
            base_only_attrs: Vec::new(),
            variants_only_attrs: Vec::new(),
            fluent_contexts,
            global_default: None,
            field_groups: std::collections::HashMap::new(),
        })
    }
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
    always_required: bool,
    always_optional: bool,
    /// Whether the type is already Option<T> (so we avoid wrapping again).
    is_option: bool,
    /// Attributes to apply when field is optional in a context
    optional_attrs: Vec<Attribute>,
    /// Attributes to apply when field is required in a context  
    required_attrs: Vec<Attribute>,
    /// Whether this field should skip default attributes
    no_default_attrs: bool,
    /// Attribute names that should only appear on the base struct field
    base_only_field_attrs: Vec<String>,
}

/// Performs the expansion of the macro.
fn expand_context_variants(cfg: VariantList, input: DeriveInput) -> Result<TokenStream2, syn::Error> {
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

    // Validate fluent contexts for field conflicts and coverage
    validate_fluent_contexts(&cfg, &all_field_names);

    // For each field, collect rules and remove our macro-specific attributes.
    let mut processed_fields = Vec::new();
    for f in fields {
        processed_fields.push(process_field(f, &cfg, &all_field_names)?);
    }

    // Validate required, optional, and never variant names exist in the variant list.
    for field_spec in &processed_fields {
        for req in &field_spec.required_in {
            if !cfg.variants.iter().any(|v| v == req) {
                return Err(syn::Error::new(req.span(), format!("unknown variant '{}' in #[ctx_required] attribute", req)));
            }
        }
        for opt in &field_spec.optional_in {
            if !cfg.variants.iter().any(|v| v == opt) {
                return Err(syn::Error::new(opt.span(), format!("unknown variant '{}' in #[ctx_optional] attribute", opt)));
            }
        }
        for never in &field_spec.never_in {
            if !cfg.variants.iter().any(|v| v == never) {
                return Err(syn::Error::new(never.span(), format!("unknown variant '{}' in #[ctx_never] attribute", never)));
            }
        }
    }

    // Validate default behavior variant names
    for req in &cfg.default_required {
        if !cfg.variants.iter().any(|v| v == req) {
            return Err(syn::Error::new(req.span(), format!("unknown variant '{}' in #[ctx_default_required] attribute", req)));
        }
    }
    for opt in &cfg.default_optional {
        if !cfg.variants.iter().any(|v| v == opt) {
            return Err(syn::Error::new(opt.span(), format!("unknown variant '{}' in #[ctx_default_optional] attribute", opt)));
        }
    }
    for never in &cfg.default_never {
        if !cfg.variants.iter().any(|v| v == never) {
            return Err(syn::Error::new(never.span(), format!("unknown variant '{}' in #[ctx_default_never] attribute", never)));
        }
    }

    // Remove macro attribute from original struct attributes and parse default attributes.
    let mut struct_attrs = Vec::new();
    let mut default_required = Vec::new();
    let mut default_optional = Vec::new();
    let mut default_never = Vec::new();
    let mut default_optional_attrs = Vec::new();
    let mut default_required_attrs = Vec::new();
    let mut base_only_attrs = Vec::new();
    let mut variants_only_attrs = Vec::new();
    
    for attr in input.attrs {
        if is_macro_attr(&attr, "context_variants") {
            // Skip the main macro attribute
            continue;
        } else if is_macro_attr(&attr, "ctx_default_required") {
            let list = parse_attribute_args(&attr)?;
            default_required.extend(list);
        } else if is_macro_attr(&attr, "ctx_default_optional") {
            let list = parse_attribute_args(&attr)?;
            default_optional.extend(list);
        } else if is_macro_attr(&attr, "ctx_default_never") {
            let list = parse_attribute_args(&attr)?;
            default_never.extend(list);
        } else if is_macro_attr(&attr, "ctx_default_optional_attrs") {
            // Parse the inner attributes for optional fields
            let attrs = parse_ctx_default_attrs_attribute(&attr)?;
            default_optional_attrs.extend(attrs);
        } else if is_macro_attr(&attr, "ctx_default_required_attrs") {
            // Parse the inner attributes for required fields
            let attrs = parse_ctx_default_attrs_attribute(&attr)?;
            default_required_attrs.extend(attrs);
        } else if is_macro_attr(&attr, "ctx_base_only") {
            // Parse attribute names that should only appear on base struct
            let attr_names = parse_attribute_name_list(&attr)?;
            base_only_attrs.extend(attr_names);
        } else if is_macro_attr(&attr, "ctx_variants_only") {
            // Parse attribute names that should only appear on variant structs
            let attr_names = parse_attribute_name_list(&attr)?;
            variants_only_attrs.extend(attr_names);
        } else {
            struct_attrs.push(attr);
        }
    }
    
    // Update the config with default behaviors
    let mut cfg = cfg;
    cfg.default_required = default_required;
    cfg.default_optional = default_optional;
    cfg.default_never = default_never;
    cfg.default_optional_attrs = default_optional_attrs;
    cfg.default_required_attrs = default_required_attrs;
    cfg.base_only_attrs = base_only_attrs;
    cfg.variants_only_attrs = variants_only_attrs;

    // Build tokens for original struct but without our field-level macros.
    let orig_fields_tokens = processed_fields.iter().map(|fs| {
        let FieldSpec { ident, ty, vis, attrs, .. } = fs;
        quote! {
            #(#attrs)*
            #vis #ident : #ty,
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
            let FieldSpec { ident, ty, vis, attrs, required_in, optional_in, never_in, always_required, always_optional, is_option, optional_attrs, required_attrs, no_default_attrs, base_only_field_attrs } = fs;
            
            // Check if this field should be excluded from this variant
            if never_in.iter().any(|v| v == variant) {
                return None; // Skip this field entirely
            }
            
            // Check if this field is marked to never appear in this variant by default
            if cfg.default_never.iter().any(|v| v == variant) && 
               !required_in.iter().any(|v| v == variant) && 
               !optional_in.iter().any(|v| v == variant) {
                return None; // Skip this field entirely
            }
            
            // Determine if field is required for this variant
            let required_here = if *always_optional {
                false
            } else if *always_required {
                true
            } else if optional_in.iter().any(|v| v == variant) {
                false
            } else if required_in.iter().any(|v| v == variant) {
                true
            } else if cfg.default_required.iter().any(|v| v == variant) {
                true
            } else if cfg.default_optional.iter().any(|v| v == variant) {
                false
            } else {
                // Default behavior: fields are optional unless explicitly required
                false
            };
            
            let ty_tokens: TokenStream2 = if required_here {
                quote! { #ty }
            } else {
                // If the original type is Option<...>, preserve it; otherwise wrap in Option
                if *is_option {
                    quote! { #ty }
                } else {
                    quote! { ::core::option::Option<#ty> }
                }
            };
            
            // Determine which conditional attributes to apply
            let mut conditional_attrs = if required_here {
                required_attrs.clone()
            } else {
                optional_attrs.clone()
            };
            
            // Add default attributes if field doesn't opt out
            if !no_default_attrs {
                if required_here {
                    conditional_attrs.extend(cfg.default_required_attrs.iter().cloned());
                } else {
                    conditional_attrs.extend(cfg.default_optional_attrs.iter().cloned());
                }
            }
            
            // Filter field attributes for variants (exclude base-only attributes)
            let variant_field_attrs: Vec<_> = attrs.iter()
                .filter(|attr| !should_exclude_field_attr_from_variants(attr, &base_only_field_attrs))
                .cloned()
                .collect();
            
            Some(quote! {
                #(#variant_field_attrs)*
                #(#conditional_attrs)*
                #vis #ident : #ty_tokens,
            })
        });

        // Copy generics and where clause
        let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();
        
        // Filter struct attributes for variants
        let variant_derive_attrs: Vec<_> = struct_attrs.iter()
            .filter(|attr| attr.path().is_ident("derive"))
            .filter(|attr| !should_exclude_from_variants(attr, &cfg))
            .cloned()
            .collect();
        let variant_other_attrs: Vec<_> = struct_attrs.iter()
            .filter(|attr| !attr.path().is_ident("derive"))
            .filter(|attr| !should_exclude_from_variants(attr, &cfg))
            .cloned()
            .collect();
            
        variant_tokens.extend(quote! {
            #(#variant_derive_attrs)*
            #(#variant_other_attrs)*
            #vis struct #variant_ident #impl_generics #where_clause {
                #(#var_fields)*
            }
        });
    }

    // Compose final tokens
    let expanded = quote! {
        #orig_struct
        #variant_tokens
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
    let mut always_required = false;
    let mut always_optional = false;
    let mut optional_attrs = Vec::new();
    let mut required_attrs = Vec::new();
    let mut other_attrs = Vec::new();
    let mut no_default_attrs = false;
    let mut base_only_field_attrs = Vec::new();
    
    // Process field attributes (old syntax)
    for attr in &field.attrs {
        if is_macro_attr(attr, "ctx_required") {
            // Parse variant list for required
            let list = parse_attribute_args(attr)?;
            required_in.extend(list);
        } else if is_macro_attr(attr, "ctx_optional") {
            let list = parse_attribute_args(attr)?;
            optional_in.extend(list);
        } else if is_macro_attr(attr, "ctx_never") {
            let list = parse_attribute_args(attr)?;
            never_in.extend(list);
        } else if is_macro_attr(attr, "ctx_always_required") {
            always_required = true;
        } else if is_macro_attr(attr, "ctx_always_optional") {
            always_optional = true;
        } else if is_macro_attr(attr, "ctx_no_default_attrs") {
            no_default_attrs = true;
        } else if is_macro_attr(attr, "ctx_base_only_attrs") {
            // Parse attribute names that should only appear on base struct field
            let attr_names = parse_attribute_name_list(attr)?;
            base_only_field_attrs.extend(attr_names);
        } else if is_macro_attr(attr, "ctx_optional_attr") {
            // Parse the inner attribute and add it to optional_attrs
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            optional_attrs.push(inner_attr);
        } else if is_macro_attr(attr, "ctx_required_attr") {
            // Parse the inner attribute and add it to required_attrs  
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            required_attrs.push(inner_attr);
        } else if is_macro_attr(attr, "when_optional") {
            // Parse the inner attribute and add it to optional_attrs
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            optional_attrs.push(inner_attr);
        } else if is_macro_attr(attr, "when_required") {
            // Parse the inner attribute and add it to required_attrs  
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            required_attrs.push(inner_attr);
        } else {
            // Keep attribute
            other_attrs.push(attr.clone());
        }
    }
    
    // Process fluent context definitions (new syntax)
    for fluent_ctx in &cfg.fluent_contexts {
        // Check if this field matches any of the required fields
        for field_ref in &fluent_ctx.required_fields {
            if field_ref.matches_field(&ident, all_field_names) {
                required_in.push(fluent_ctx.name.clone());
                break;
            }
        }
        
        // Check if this field matches any of the optional fields
        for field_ref in &fluent_ctx.optional_fields {
            if field_ref.matches_field(&ident, all_field_names) {
                optional_in.push(fluent_ctx.name.clone());
                break;
            }
        }
        
        // Check if this field matches any of the excluded fields
        for field_ref in &fluent_ctx.excluded_fields {
            if field_ref.matches_field(&ident, all_field_names) {
                never_in.push(fluent_ctx.name.clone());
                break;
            }
        }
    }
    
    // Apply default behaviors for fields not explicitly specified in fluent contexts
    for fluent_ctx in &cfg.fluent_contexts {
        let field_explicitly_mentioned = fluent_ctx.required_fields.iter().any(|field_ref| field_ref.matches_field(&ident, all_field_names)) ||
                                         fluent_ctx.optional_fields.iter().any(|field_ref| field_ref.matches_field(&ident, all_field_names)) ||
                                         fluent_ctx.excluded_fields.iter().any(|field_ref| field_ref.matches_field(&ident, all_field_names));
        
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
        always_required,
        always_optional,
        is_option,
        optional_attrs,
        required_attrs,
        no_default_attrs,
        base_only_field_attrs,
    })
}

/// Check if an attribute matches our macro attribute name.
fn is_macro_attr(attr: &Attribute, name: &str) -> bool {
    attr.path().is_ident(name)
}

/// Parse attribute arguments (e.g. `#[ctx_required(Get, Post)]`) into a list of identifiers.
fn parse_attribute_args(attr: &Attribute) -> Result<Vec<Ident>, syn::Error> {
    let meta = attr.meta.clone();
    match meta {
        Meta::List(list) => {
            let mut idents = Vec::new();
            let nested: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]> = 
                list.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;
            for meta in nested {
                match meta {
                    Meta::Path(path) => {
                        if let Some(id) = path.get_ident() {
                            idents.push(id.clone());
                        } else {
                            return Err(syn::Error::new(path.span(), "expected identifier in attribute list"));
                        }
                    }
                    _ => {
                        return Err(syn::Error::new(meta.span(), "expected identifier in attribute list"));
                    }
                }
            }
            if idents.is_empty() {
                return Err(syn::Error::new(list.span(), "attribute list cannot be empty"));
            }
            Ok(idents)
        }
        _ => {
            Err(syn::Error::new(meta.span(), "expected a list of identifiers"))
        }
    }
}

/// Parse attribute names for ctx_base_only and ctx_variants_only
/// Example: #[ctx_base_only(derive, table_name, sqlx)]
fn parse_attribute_name_list(attr: &Attribute) -> Result<Vec<String>, syn::Error> {
    let meta = attr.meta.clone();
    match meta {
        Meta::List(list) => {
            let mut names = Vec::new();
            let nested: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]> = 
                list.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;
            for meta in nested {
                match meta {
                    Meta::Path(path) => {
                        if let Some(ident) = path.get_ident() {
                            names.push(ident.to_string());
                        } else {
                            // Handle multi-segment paths like "sqlx::FromRow"
                            let path_str = path.segments.iter()
                                .map(|seg| seg.ident.to_string())
                                .collect::<Vec<_>>()
                                .join("::");
                            names.push(path_str);
                        }
                    }
                    Meta::NameValue(nv) => {
                        if let Some(ident) = nv.path.get_ident() {
                            names.push(ident.to_string());
                        }
                    }
                    Meta::List(inner_list) => {
                        if let Some(ident) = inner_list.path.get_ident() {
                            names.push(ident.to_string());
                        }
                    }
                }
            }
            Ok(names)
        }
        _ => {
            Err(syn::Error::new(meta.span(), "expected a list of attribute names"))
        }
    }
}

/// Check if a field attribute should be excluded from variant structs
fn should_exclude_field_attr_from_variants(attr: &Attribute, base_only_attrs: &[String]) -> bool {
    // Check if this attribute matches any base-only patterns
    for base_only in base_only_attrs {
        if attr_matches_pattern(attr, base_only) {
            return true;
        }
    }
    false
}

/// Check if an attribute should be excluded from variant structs
fn should_exclude_from_variants(attr: &Attribute, cfg: &VariantList) -> bool {
    // Check if this attribute matches any base-only patterns
    for base_only in &cfg.base_only_attrs {
        if attr_matches_pattern(attr, base_only) {
            return true;
        }
    }
    false
}

/// Check if an attribute matches a given pattern
fn attr_matches_pattern(attr: &Attribute, pattern: &str) -> bool {
    // Simple path matching
    if let Some(ident) = attr.path().get_ident() {
        return ident.to_string() == pattern;
    }
    
    // Multi-segment path matching
    let path_str = attr.path().segments.iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");
    
    path_str == pattern || path_str.ends_with(&format!("::{}", pattern))
}

/// Parse ctx_default_optional_attrs or ctx_default_required_attrs to extract multiple inner attributes.
/// Example: #[ctx_default_optional_attrs(serde(skip_serializing_if = "Option::is_none"), doc = "Optional field")]
/// Should extract: [#[serde(skip_serializing_if = "Option::is_none")], #[doc = "Optional field"]]
fn parse_ctx_default_attrs_attribute(attr: &Attribute) -> Result<Vec<Attribute>, syn::Error> {
    let meta = attr.meta.clone();
    match meta {
        Meta::List(list) => {
            let mut attributes = Vec::new();
            let nested: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]> = 
                list.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;
            
            for inner_meta in nested {
                // Convert Meta back to Attribute
                attributes.push(Attribute {
                    pound_token: syn::Token![#](attr.span()),
                    style: syn::AttrStyle::Outer,
                    bracket_token: syn::token::Bracket(attr.span()),
                    meta: inner_meta,
                });
            }
            Ok(attributes)
        }
        _ => {
            Err(syn::Error::new(meta.span(), "expected a list with inner attributes"))
        }
    }
}

/// Parse ctx_optional_attr or ctx_required_attr to extract the inner attribute.
/// Example: #[ctx_optional_attr(serde(skip_serializing_if = "Option::is_none"))]
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
    let mut variants_cfg = match parse_mixed_args(args) {
        Ok(cfg) => cfg,
        Err(err) => return err.into_compile_error().into(),
    };

    // Expand field groups in fluent contexts
    if let Err(err) = expand_field_groups(&mut variants_cfg) {
        return err.into_compile_error().into();
    }

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
        default_required: Vec::new(),
        default_optional: Vec::new(),
        default_never: Vec::new(),
        default_optional_attrs: Vec::new(),
        default_required_attrs: Vec::new(),
        base_only_attrs: Vec::new(),
        variants_only_attrs: Vec::new(),
        fluent_contexts,
        global_default: global_default,
        field_groups,
    })
}

/// Parse groups expression: auth(user_id, token), contact(name, email)
fn parse_groups_expression(expr: &syn::Expr) -> Result<std::collections::HashMap<String, Vec<Ident>>, syn::Error> {
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

/// Parse a single group: auth(user_id, token)
fn parse_single_group(call: &syn::ExprCall) -> Result<(String, Vec<Ident>), syn::Error> {
    // Get group name
    let group_name = match call.func.as_ref() {
        syn::Expr::Path(path) => {
            path.path.get_ident()
                .ok_or_else(|| syn::Error::new(path.span(), "expected group name"))?
                .to_string()
        }
        _ => return Err(syn::Error::new(call.func.span(), "expected group name")),
    };
    
    // Parse field list
    let mut fields = Vec::new();
    for arg in &call.args {
        match arg {
            syn::Expr::Path(path) => {
                if let Some(ident) = path.path.get_ident() {
                    fields.push(ident.clone());
                } else {
                    return Err(syn::Error::new(arg.span(), "expected field name"));
                }
            }
            _ => return Err(syn::Error::new(arg.span(), "expected field name")),
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

/// Expand field groups in fluent contexts
fn expand_field_groups(variants_cfg: &mut VariantList) -> Result<(), syn::Error> {
    // For each fluent context, expand group names to individual field names
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
                FieldRef::AllFields { .. } => {
                    // Keep all_fields() as-is (it will be resolved later when we have struct field access)
                    expanded_required.push(field_ref.clone());
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
                FieldRef::AllFields { .. } => {
                    // Keep all_fields() as-is
                    expanded_optional.push(field_ref.clone());
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
                FieldRef::AllFields { .. } => {
                    // Keep all_fields() as-is
                    expanded_excluded.push(field_ref.clone());
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
                if field_ref.matches_field(field_name, all_field_names) {
                    let mentions = field_mentions.entry(field_name.clone()).or_insert_with(Vec::new);
                    mentions.push("required");
                }
            }
        }
        
        for field_ref in &fluent_ctx.optional_fields {
            for field_name in all_field_names {
                if field_ref.matches_field(field_name, all_field_names) {
                    let mentions = field_mentions.entry(field_name.clone()).or_insert_with(Vec::new);
                    mentions.push("optional");
                }
            }
        }
        
        for field_ref in &fluent_ctx.excluded_fields {
            for field_name in all_field_names {
                if field_ref.matches_field(field_name, all_field_names) {
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