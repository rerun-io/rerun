use proc_macro2::TokenStream;
use quote::quote;

use super::{doc_comment, NEWLINE_TOKEN};

/// A Cpp struct/class method.
pub struct Method {
    pub doc_string: String,
    pub declaration: TokenStream,
    pub definition_body: TokenStream,
    pub inline: bool,
}

impl Default for Method {
    fn default() -> Self {
        Self {
            doc_string: String::new(),
            declaration: TokenStream::new(),
            definition_body: TokenStream::new(),
            inline: true,
        }
    }
}

impl Method {
    pub fn to_hpp_tokens(&self) -> TokenStream {
        let Self {
            doc_string,
            declaration,
            definition_body,
            inline: is_inline,
        } = self;

        let quoted_doc = if doc_string.is_empty() {
            quote! {}
        } else {
            doc_comment(doc_string)
        };
        if *is_inline {
            quote! {
                #NEWLINE_TOKEN
                #quoted_doc
                #declaration {
                    #definition_body
                }
            }
        } else {
            quote! {
                #NEWLINE_TOKEN
                #quoted_doc
                #declaration;
            }
        }
    }

    pub fn to_cpp_tokens(&self) -> TokenStream {
        let Self {
            doc_string: _,
            declaration,
            definition_body,
            inline,
        } = self;

        if *inline {
            quote! {}
        } else {
            quote! {
                #declaration {
                    #definition_body
                }
            }
        }
    }
}
