use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::NEWLINE_TOKEN;

/// A C++ forward declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum ForwardDecl {
    Struct(String),
    Class(String),
}

impl quote::ToTokens for ForwardDecl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ForwardDecl::Struct(name) => {
                let name_ident = format_ident!("{name}");
                quote! { struct #name_ident; }
            }
            ForwardDecl::Class(name) => {
                let name_ident = format_ident!("{name}");
                quote! { class #name_ident; }
            }
        }
        .to_tokens(tokens);
    }
}

/// Keeps track of necessary forward decls for a file.
#[derive(Default)]
pub struct ForwardDecls {
    /// E.g. `DataType` in `arrow` etc.
    declarations_per_namespace: BTreeMap<String, BTreeSet<ForwardDecl>>,
}

impl ForwardDecls {
    #[allow(dead_code)]
    pub fn insert(&mut self, namespace: impl Into<String>, decl: ForwardDecl) {
        self.declarations_per_namespace
            .entry(namespace.into())
            .or_default()
            .insert(decl);
    }
}

impl quote::ToTokens for ForwardDecls {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            declarations_per_namespace,
        } = self;

        let declarations = declarations_per_namespace
            .iter()
            .map(|(namespace, declarations)| {
                let namespace_ident = format_ident!("{namespace}");
                quote! {
                    #NEWLINE_TOKEN
                    namespace #namespace_ident {
                        #(#declarations)*
                    }
                }
            });

        quote! {
            #NEWLINE_TOKEN
            #(#declarations)*
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
        }
        .to_tokens(tokens);
    }
}
