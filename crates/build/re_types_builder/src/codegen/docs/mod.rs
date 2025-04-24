mod datatype_docs;
mod snippets_ref;
mod website;

pub use self::{
    datatype_docs::datatype_docs, snippets_ref::SnippetsRefCodeGenerator,
    website::DocsCodeGenerator,
};
