use proc_macro2::TokenStream;
use quote::quote;

use super::{NEWLINE_TOKEN, lines_from_docs, quote_doc_comment, quote_doc_lines};
use crate::{Docs, Objects, Reporter};

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

    pub fn to_cpp_tokens(&self, class_or_struct_name: &TokenStream) -> TokenStream {
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

#[derive(Default)]
pub enum MethodDocumentation {
    #[default]
    None,
    String(String),
    Docs(Docs),
}

impl From<Docs> for MethodDocumentation {
    fn from(d: Docs) -> Self {
        Self::Docs(d)
    }
}

impl From<String> for MethodDocumentation {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for MethodDocumentation {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

impl MethodDocumentation {
    fn quoted(&self, reporter: &Reporter, objects: &Objects) -> TokenStream {
        match self {
            Self::None => {
                quote!()
            }
            Self::String(s) => {
                let lines = s.lines().map(quote_doc_comment);
                quote! {
                    #(#lines)*
                }
            }
            Self::Docs(docs) => {
                let lines = lines_from_docs(reporter, objects, docs);
                quote_doc_lines(&lines)
            }
        }
    }
}

/// A Cpp struct/class method.
pub struct Method {
    pub docs: MethodDocumentation,
    pub declaration: MethodDeclaration,
    pub definition_body: TokenStream,
    pub inline: bool,
}

impl Default for Method {
    fn default() -> Self {
        Self {
            docs: MethodDocumentation::None,
            declaration: MethodDeclaration::default(),
            definition_body: TokenStream::new(),
            inline: true,
        }
    }
}

impl Method {
    pub fn to_hpp_tokens(&self, reporter: &Reporter, objects: &Objects) -> TokenStream {
        let Self {
            docs,
            declaration,
            definition_body,
            inline: is_inline,
        } = self;

        let docs = docs.quoted(reporter, objects);
        let declaration = declaration.to_hpp_tokens();
        if *is_inline {
            quote! {
                #NEWLINE_TOKEN
                #docs
                #declaration {
                    #definition_body
                }
                #NEWLINE_TOKEN
            }
        } else {
            quote! {
                #NEWLINE_TOKEN
                #docs
                #declaration;
                #NEWLINE_TOKEN
            }
        }
    }

    pub fn to_cpp_tokens(&self, class_or_struct_name: &TokenStream) -> TokenStream {
        let Self {
            docs: _,
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
