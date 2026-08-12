// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A path to an entity in the `ChunkStore`.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "str")]
#[python(array_aliases = "Sequence[str]")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord, Default, Hash))]
#[rust(override_crate = "re_types_core")]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct EntityPath {
    // TODO(jleibs): This should be a special primitive
    pub path: String,
}
