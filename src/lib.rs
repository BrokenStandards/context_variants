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
//! See the crate level documentation and the tests for usage examples.

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, Attribute, DeriveInput, Field, Fields, Lit, Meta, Type, Visibility, parse::Parse, parse::ParseStream};

/// The main attribute macro. See crate level docs for details.
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

/// Custom parser for attribute arguments
struct AttributeArgs {
    metas: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>,
}

impl Parse for AttributeArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let metas = input.parse_terminated(Meta::parse, syn::Token![,])?;
        Ok(AttributeArgs { metas })
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
}

impl VariantList {
    fn from_args(args: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>) -> Result<Self, syn::Error> {
        let mut variants = Vec::new();
        let mut prefix = None;
        let mut suffix = None;
        for meta in args {
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
                        let lit = match nv.value {
                            syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(ref s), .. }) => s.value(),
                            _ => {
                                return Err(syn::Error::new(nv.value.span(), "expected a string literal for prefix/suffix"));
                            }
                        };
                        match ident.to_string().as_str() {
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
                                return Err(syn::Error::new(ident.span(), "unknown argument; expected prefix or suffix"));
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
        if variants.is_empty() {
            return Err(syn::Error::new(Span::call_site(), "no variants specified"));
        }
        Ok(VariantList { 
            variants, 
            prefix, 
            suffix, 
            default_required: Vec::new(),
            default_optional: Vec::new(),
            default_never: Vec::new(),
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

    // For each field, collect rules and remove our macro-specific attributes.
    let mut processed_fields = Vec::new();
    for f in fields {
        processed_fields.push(process_field(f, &cfg)?);
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
        } else {
            struct_attrs.push(attr);
        }
    }
    
    // Update the config with default behaviors
    let mut cfg = cfg;
    cfg.default_required = default_required;
    cfg.default_optional = default_optional;
    cfg.default_never = default_never;

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
            let FieldSpec { ident, ty, vis, attrs, required_in, optional_in, never_in, always_required, always_optional, is_option, optional_attrs, required_attrs } = fs;
            
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
                // Default behavior: preserve original type (required if not Option, optional if Option)
                !is_option
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
            let conditional_attrs = if required_here {
                required_attrs
            } else {
                optional_attrs
            };
            
            Some(quote! {
                #(#attrs)*
                #(#conditional_attrs)*
                #vis #ident : #ty_tokens,
            })
        });

        // Copy generics and where clause
        let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();
        // Copy derive attributes from original struct, excluding context_variants and field-level macro attributes. We'll clone all derive attributes that are on the original struct.
        let derive_attrs: Vec<_> = struct_attrs.iter().filter(|attr| attr.path().is_ident("derive")).cloned().collect();
        let other_attrs: Vec<_> = struct_attrs.iter().filter(|attr| !attr.path().is_ident("derive")).cloned().collect();
        variant_tokens.extend(quote! {
            #(#derive_attrs)*
            #(#other_attrs)*
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
fn process_field(field: &Field, _cfg: &VariantList) -> Result<FieldSpec, syn::Error> {
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
        } else if is_macro_attr(attr, "ctx_optional_attr") {
            // Parse the inner attribute and add it to optional_attrs
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            optional_attrs.push(inner_attr);
        } else if is_macro_attr(attr, "ctx_required_attr") {
            // Parse the inner attribute and add it to required_attrs  
            let inner_attr = parse_ctx_attr_attribute(attr)?;
            required_attrs.push(inner_attr);
        } else {
            // Keep attribute
            other_attrs.push(attr.clone());
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