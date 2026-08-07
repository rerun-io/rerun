// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A log entry in a text log, comprised of a text body and its log level.
///
/// \example archetypes/text_log_integration text="Logging text directly or via a logger" image="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Text")]
#[docs(view_types = "TextLogView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "TextLog")]
#[rust(derive(PartialEq))]
pub struct TextLog {
    /// The body of the message.
    #[rerun(required)]
    pub text: rerun::components::Text,

    /// The verbosity level of the message.
    ///
    /// This can be used to filter the log messages in the Rerun Viewer.
    #[rerun(recommended)]
    pub level: Option<rerun::components::TextLogLevel>,

    /// Optional color to use for the log line in the Rerun Viewer.
    #[rerun(optional)]
    pub color: Option<rerun::components::Color>,
}
