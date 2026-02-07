use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use super::{NEWLINE_TOKEN, quote_hide_from_docs};

/// A C++ forward declaration.
#[derive(Debug, Clone)]
pub enum ForwardDecl {
    Class(Ident),
    TemplateClass(Ident),

    /// Aliases are only identified by their `from` name!
    Alias {
        from: Ident,
        to: TokenStream,
    },
}

impl PartialEq for ForwardDecl {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Class(l0), Self::Class(r0))
            | (Self::TemplateClass(l0), Self::TemplateClass(r0)) => l0 == r0,
            (Self::Alias { from: l_from, .. }, Self::Alias { from: r_from, .. }) => {
                // Ignore `to` for equality
                l_from == r_from
            }
            _ => false,
        }
    }
}

impl Eq for ForwardDecl {}

impl PartialOrd for ForwardDecl {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ForwardDecl {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::TemplateClass(a), Self::TemplateClass(b))
            | (Self::Class(a), Self::Class(b))
            | (Self::Alias { from: a, .. }, Self::Alias { from: b, .. }) => {
                a.to_string().cmp(&b.to_string())
            }
            (Self::TemplateClass(_), _) => std::cmp::Ordering::Less,
            (_, Self::TemplateClass(_)) => std::cmp::Ordering::Greater,

            (Self::Class(_), _) => std::cmp::Ordering::Less,
            (_, Self::Class(_)) => std::cmp::Ordering::Greater,
        }
    }
}

impl quote::ToTokens for ForwardDecl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Class(name) => {
                quote! { class #name; }
            }
            Self::TemplateClass(name) => {
                // Doxygen likes including template declarations in the docs.
                let hide_from_docs = quote_hide_from_docs();
                quote! {
                    #hide_from_docs
                    template<typename T> class #name;
                    #NEWLINE_TOKEN
                    #NEWLINE_TOKEN
                }
            }
            Self::Alias { from, to } => {
                quote! { using #from = #to; }
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
