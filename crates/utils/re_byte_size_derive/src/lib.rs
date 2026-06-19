//! Derive macro for the `SizeBytes` trait from `re_byte_size`.
//!
//! ```ignore
//! use ::re_byte_size::SizeBytes;
//! use re_byte_size_derive::SizeBytes;
//!
//! #[derive(SizeBytes)]
//! struct Foo {
//!     name: String,
//!     values: Vec<u32>,
//!
//!     #[size_bytes(ignore)]
//!     cache: Vec<u8>,
//! }
//! ```

use std::cell::OnceCell;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Data, DataEnum, DeriveInput, Field, Fields, GenericParam, Generics, Index, Member, Type,
    parse_macro_input,
};

thread_local! {
    /// The resolved path to the `SizeBytes` trait, as text, for the crate currently being compiled.
    static SIZE_BYTES_PATH: OnceCell<Option<String>> = const { OnceCell::new() };
}

/// The path the generated code uses to name the `SizeBytes` trait.
///
/// Internal crates depend on `re_byte_size` directly, so `::re_byte_size::SizeBytes` works.
/// External users only depend on `rerun`, which re-exports the trait as `rerun::SizeBytes`.
/// Errors at `span` when the consuming crate depends on neither.
fn size_bytes_path(span: Span) -> syn::Result<TokenStream2> {
    let path = SIZE_BYTES_PATH.with(|cell| cell.get_or_init(resolve_size_bytes_path).clone());
    match path {
        Some(path) => Ok(path
            .parse()
            .expect("resolved trait path should be valid tokens")),
        None => Err(syn::Error::new(
            span,
            "`SizeBytes` trait not found in default locations, either manually set location with `#[size_bytes(crate_root = some::path)]`, or depend on `rerun` or `re_byte_size`",
        )),
    }
}

fn resolve_size_bytes_path() -> Option<String> {
    let found = crate_name("re_byte_size")
        .ok()
        .or_else(|| crate_name("rerun").ok())?;
    let krate = match found {
        FoundCrate::Itself => "crate".to_owned(),
        FoundCrate::Name(name) => format!("::{name}"),
    };
    Some(format!("{krate}::SizeBytes"))
}

/// Derives `SizeBytes` for a struct or enum.
///
/// The generated `heap_size_bytes` sums the heap size of every field. The associated `const IS_POD`
/// is `true` only when every field type is itself POD, in which case `heap_size_bytes`
/// short-circuits to `0`.
///
/// Annotate a field with `#[size_bytes(ignore)]` to leave it out of both the sum and the `IS_POD`
/// computation.
///
/// Annotate the type with `#[size_bytes(profile)]` to insert a `re_tracing::profile_function!()`
/// at the top of the generated `heap_size_bytes`. The consuming crate must depend on `re_tracing`.
///
/// By default the macro finds the `re_byte_size` crate by reading the consuming crate's `Cargo.toml`.
/// Annotate the type with `#[size_bytes(crate_root = some::path::to::re_byte_size)]` to name it
/// directly instead, skipping that lookup.
#[proc_macro_derive(SizeBytes, attributes(size_bytes))]
pub fn derive_size_bytes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let options = parse_type_options(&input.attrs)?;

    // An explicit `crate_root` sidesteps the (somewhat costly) `Cargo.toml` lookup.
    let trait_path = match &options.crate_root {
        Some(crate_root) => quote! { #crate_root::SizeBytes },
        None => size_bytes_path(name.span())?,
    };

    let generics = add_trait_bounds(input.generics.clone(), &trait_path);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (is_pod, heap_size_body) = match &input.data {
        Data::Struct(data) => struct_body(&data.fields, &trait_path)?,
        Data::Enum(data) => enum_body(data, &trait_path)?,
        Data::Union(_) => {
            return Err(syn::Error::new(
                input.ident.span(),
                "`SizeBytes` cannot be derived for unions",
            ));
        }
    };

    let profile_stmt = if options.profile {
        quote! { ::re_tracing::profile_function!(); }
    } else {
        quote!()
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #trait_path for #name #ty_generics #where_clause {
            const IS_POD: bool = #is_pod;

            fn heap_size_bytes(&self) -> u64 {
                #profile_stmt
                #heap_size_body
            }
        }
    })
}

/// Type-level `#[size_bytes(...)]` options.
#[derive(Default)]
struct TypeOptions {
    /// Insert a `re_tracing::profile_function!()` at the top of `heap_size_bytes`.
    profile: bool,

    /// Path to the `re_byte_size` crate, overriding the automatic lookup.
    crate_root: Option<syn::Path>,
}

fn parse_type_options(attrs: &[syn::Attribute]) -> syn::Result<TypeOptions> {
    let mut options = TypeOptions::default();
    for attr in attrs {
        if !attr.path().is_ident("size_bytes") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("profile") {
                options.profile = true;
                Ok(())
            } else if meta.path.is_ident("crate_root") {
                options.crate_root = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unknown `size_bytes` option, expected `profile` or `crate_root`"))
            }
        })?;
    }
    Ok(options)
}

/// Builds the `IS_POD` expression and `heap_size_bytes` body for a struct.
fn struct_body(
    fields: &Fields,
    trait_path: &TokenStream2,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    let mut members = Vec::new();
    let mut types = Vec::new();
    let mut ignored = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let member = member_for(index, field);
        if is_ignored(field)? {
            ignored.push(member);
            continue;
        }
        members.push(member);
        types.push(&field.ty);
    }

    let is_pod = all_pod_expr(&types, trait_path);

    let terms: Vec<TokenStream2> = std::iter::zip(&members, &types)
        .map(|(member, ty)| {
            quote! {
                (if <#ty as #trait_path>::IS_POD {
                    0
                } else {
                    #trait_path::heap_size_bytes(&self.#member)
                })
            }
        })
        .collect();
    let sum = sum_expr(&terms);

    let body = quote! {
        // Read the ignored fields so they don't trip the `dead_code` lint.
        #( let _ = &self.#ignored; )*
        #sum
    };

    Ok((is_pod, body))
}

/// Builds the `IS_POD` expression and `heap_size_bytes` body for an enum.
fn enum_body(
    data: &DataEnum,
    trait_path: &TokenStream2,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    let mut all_types: Vec<&Type> = Vec::new();
    let mut arms: Vec<TokenStream2> = Vec::new();

    for variant in &data.variants {
        let variant_ident = &variant.ident;
        let mut bindings: Vec<Ident> = Vec::new();
        let mut binding_types: Vec<&Type> = Vec::new();

        let pattern = match &variant.fields {
            Fields::Named(named) => {
                let mut patterns = Vec::new();
                for field in &named.named {
                    let ident = field.ident.clone().expect("named field has an identifier");
                    if is_ignored(field)? {
                        // Bind to an underscore name so the field is read (no `dead_code`) but
                        // still counts as unused (no `unused_variables`).
                        let binding = format_ident!("_{}", ident);
                        patterns.push(quote! { #ident: #binding });
                        continue;
                    }
                    all_types.push(&field.ty);
                    binding_types.push(&field.ty);
                    patterns.push(quote! { #ident });
                    bindings.push(ident);
                }
                quote! { Self::#variant_ident { #(#patterns),* } }
            }
            Fields::Unnamed(unnamed) => {
                let mut patterns = Vec::new();
                for (index, field) in unnamed.unnamed.iter().enumerate() {
                    if is_ignored(field)? {
                        let binding = format_ident!("_field_{}", index);
                        patterns.push(quote! { #binding });
                        continue;
                    }
                    all_types.push(&field.ty);
                    binding_types.push(&field.ty);
                    let binding = format_ident!("field_{}", index);
                    patterns.push(quote! { #binding });
                    bindings.push(binding);
                }
                quote! { Self::#variant_ident( #(#patterns),* ) }
            }
            Fields::Unit => quote! { Self::#variant_ident },
        };

        let terms: Vec<TokenStream2> = std::iter::zip(&bindings, &binding_types)
            .map(|(binding, ty)| {
                quote! {
                    (if <#ty as #trait_path>::IS_POD {
                        0
                    } else {
                        #trait_path::heap_size_bytes(#binding)
                    })
                }
            })
            .collect();
        let sum = sum_expr(&terms);
        arms.push(quote! { #pattern => #sum, });
    }

    let is_pod = all_pod_expr(&all_types, trait_path);

    let body = if data.variants.is_empty() {
        quote! { 0 }
    } else {
        quote! {
            match self {
                #(#arms)*
            }
        }
    };

    Ok((is_pod, body))
}

/// The accessor used to reach a field through `self`, e.g. `name` or `0`.
fn member_for(index: usize, field: &Field) -> Member {
    match &field.ident {
        Some(ident) => Member::Named(ident.clone()),
        None => Member::Unnamed(Index::from(index)),
    }
}

/// A `bool` expression that is `true` only when every given type is POD.
fn all_pod_expr(types: &[&Type], trait_path: &TokenStream2) -> TokenStream2 {
    match types.split_first() {
        None => quote! { true },
        Some((first, rest)) => quote! {
            <#first as #trait_path>::IS_POD
            #( && <#rest as #trait_path>::IS_POD )*
        },
    }
}

/// Joins the heap-size terms with `+`, or `0` when there are none.
fn sum_expr(terms: &[TokenStream2]) -> TokenStream2 {
    match terms.split_first() {
        None => quote! { 0 },
        Some((first, rest)) => quote! { #first #( + #rest )* },
    }
}

/// Bounds every generic type parameter with `SizeBytes`.
fn add_trait_bounds(mut generics: Generics, trait_path: &TokenStream2) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(syn::parse_quote!(#trait_path));
        }
    }
    generics
}

/// Whether a field carries `#[size_bytes(ignore)]`.
fn is_ignored(field: &Field) -> syn::Result<bool> {
    let mut ignored = false;
    for attr in &field.attrs {
        if !attr.path().is_ident("size_bytes") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("ignore") {
                ignored = true;
                Ok(())
            } else {
                Err(meta.error("unknown `size_bytes` option, expected `ignore`"))
            }
        })?;
    }
    Ok(ignored)
}
