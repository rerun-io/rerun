// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view of a single text document, for use with [archetypes.TextDocument].
///
/// \example views/text_document title="Use a blueprint to show a text document." image="https://static.rerun.io/text_log/27f15235fe9639ff42b6ea0d2f0ce580685c021c/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "TextDocument")]
#[rerun(state = "unstable")]
pub struct TextDocumentView {
    /// Formatting options for the text document view.
    pub format_options: rerun::blueprint::archetypes::TextDocumentFormat,
}
