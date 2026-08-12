// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A string of text, encoded as UTF-8.
//
// NOTE: Apache Arrow uses UTF-8 encoding of its String type, as does Rust.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str] | npt.ArrayLike")]
#[rust(derive_only(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(override_crate = "re_types_core")]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct Utf8 {
    pub value: String,
}
