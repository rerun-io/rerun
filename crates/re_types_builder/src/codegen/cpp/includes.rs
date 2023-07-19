use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;

use super::{NEWLINE_TOKEN, SYS_INCLUDE_PATH_PREFIX_TOKEN, SYS_INCLUDE_PATH_SUFFIX_TOKEN};

/// Keeps track of necessary includes for a file.
#[derive(Default)]
pub struct Includes {
    /// `#include <vector>` etc
    pub system: BTreeSet<String>,

    /// `#include datatypes.hpp"` etc
    pub local: BTreeSet<String>,
}

impl quote::ToTokens for Includes {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { system, local } = self;

        let hash = quote! { # };
        let system = system.iter().map(|name| {
            // Need to mark system includes with tokens since they are usually not idents (can contain slashes and dots)
            quote! { #hash include #SYS_INCLUDE_PATH_PREFIX_TOKEN #name #SYS_INCLUDE_PATH_SUFFIX_TOKEN #NEWLINE_TOKEN }
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
