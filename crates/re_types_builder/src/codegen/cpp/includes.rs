use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::NEWLINE_TOKEN;

/// Keeps track of necessary includes for a file.
pub struct Includes {
    /// `#include <vector>` etc
    pub system: BTreeSet<String>,

    /// `#include datatypes.hpp"` etc
    pub local: BTreeSet<String>,
}

impl Default for Includes {
    fn default() -> Self {
        let mut slf = Self {
            system: BTreeSet::new(),
            local: BTreeSet::new(),
        };
        slf.system.insert("cstdint".to_owned()); // we use `uint32_t` etc everywhere.
        slf
    }
}

impl quote::ToTokens for Includes {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { system, local } = self;

        let hash = quote! { # };
        let system = system.iter().map(|name| {
            let name = format_ident!("{}", name);
            quote! { #hash include <#name> #NEWLINE_TOKEN }
        });
        let local = local.iter().map(|name| {
            quote! { #hash include #name #NEWLINE_TOKEN }
        });

        quote! {
            #(#system)*
            #NEWLINE_TOKEN
            #(#local)*
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
        }
        .to_tokens(tokens);
    }
}
