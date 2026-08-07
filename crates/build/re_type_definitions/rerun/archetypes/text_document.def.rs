// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A text element intended to be displayed in its own text box.
///
/// Supports raw text and markdown.
///
/// \example archetypes/text_document title="Markdown text document" image="https://static.rerun.io/textdocument/babda19558ee32ed8d730495b595aee7a5e2c174/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Text")]
#[docs(view_types = "TextDocumentView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "TextDocument")]
#[rust(derive(PartialEq))]
pub struct TextDocument {
    /// Contents of the text document.
    #[rerun(required)]
    pub text: rerun::components::Text,

    /// The Media Type of the text.
    ///
    /// For instance:
    /// * `text/plain`
    /// * `text/markdown`
    ///
    /// If omitted, `text/plain` is assumed.
    #[rerun(optional)]
    pub media_type: Option<rerun::components::MediaType>,
}
