// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Formatting options for the text document view.
///
/// These options only apply to plain text documents and have no effect on Markdown documents.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TextDocumentFormat {
    /// Whether to use a monospace font for the document body.
    ///
    /// Defaults to disabled.
    #[rerun(optional)]
    pub monospace: Option<rerun::blueprint::components::Enabled>,

    /// Whether to wrap long lines in the document body.
    ///
    /// Defaults to enabled.
    #[rerun(optional)]
    pub word_wrap: Option<rerun::blueprint::components::Enabled>,
}
