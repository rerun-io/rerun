// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A display name, typically for an entity or a item like a plot series.
///
/// This name is only a display label, never an identifier: it is not used to look anything
/// up, and two items may share the same name.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(state = "stable")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct Name {
    pub value: rerun::encodings::Utf8,
}
