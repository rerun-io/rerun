// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The severity level of a text log message.
///
/// Recommended to be one of:
/// * `"CRITICAL"`
/// * `"ERROR"`
/// * `"WARN"`
/// * `"INFO"`
/// * `"DEBUG"`
/// * `"TRACE"`
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct TextLogLevel {
    pub value: rerun::datatypes::Utf8,
}
