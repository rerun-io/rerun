// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration of the text log rows.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TextLogFormat {
    /// Whether to use a monospace font for the log message body.
    ///
    /// Defaults to not being enabled.
    #[rerun(optional)]
    pub monospace_body: Option<rerun::blueprint::components::Enabled>,
}
