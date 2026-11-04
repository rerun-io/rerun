// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The name of a layer of a segment in a dataset.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(state = "unstable")]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(repr = "transparent")]
pub struct LayerName {
    pub value: rerun::encodings::Utf8,
}
