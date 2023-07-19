use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::{doc_comment, NEWLINE_TOKEN};

#[derive(Default)]
pub struct MethodDeclaration {
    pub is_static: bool,
    pub return_type: TokenStream,
    pub name_and_parameters: TokenStream,
}

impl MethodDeclaration {
    pub fn constructor(declaration: TokenStream) -> Self {
        Self {
            is_static: false,
            return_type: TokenStream::new(),
            name_and_parameters: declaration,
        }
    }

    pub fn to_hpp_tokens(&self) -> TokenStream {
        let Self {
            is_static,
            return_type,
            name_and_parameters,
        } = self;

        let modifiers = if *is_static {
            quote! { static }
        } else {
            quote! {}
        };
        quote! {
            #modifiers #return_type #name_and_parameters
        }
    }

    pub fn to_cpp_tokens(&self, class_or_struct_name: &Ident) -> TokenStream {
        let Self {
            is_static: _,
            return_type,
            name_and_parameters,
        } = self;

        quote! {
            #return_type #class_or_struct_name::#name_and_parameters
        }
    }
}

/// A Cpp struct/class method.
pub struct Method {
    pub doc_string: String,
    pub declaration: MethodDeclaration,
    pub definition_body: TokenStream,
    pub inline: bool,
}

impl Default for Method {
    fn default() -> Self {
        Self {
            doc_string: String::new(),
            declaration: MethodDeclaration::default(),
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
        let declaration = declaration.to_hpp_tokens();
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

    pub fn to_cpp_tokens(&self, class_or_struct_name: &Ident) -> TokenStream {
        let Self {
            doc_string: _,
            declaration,
            definition_body,
            inline,
        } = self;

        let declaration = declaration.to_cpp_tokens(class_or_struct_name);
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
