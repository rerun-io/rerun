// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A timeline identified by its name.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct TimelineName {
    pub value: rerun::datatypes::Utf8,
}
