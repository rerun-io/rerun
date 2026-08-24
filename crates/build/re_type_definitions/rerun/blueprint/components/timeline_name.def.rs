// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A timeline identified by its name.
///
/// The name is used both as an identifier and as a display label: it is what timelines are
/// keyed on, and also what the user reads.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct TimelineName {
    pub value: rerun::encodings::Utf8,
}
