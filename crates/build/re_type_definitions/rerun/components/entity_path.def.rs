// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A path to an entity, usually to reference some data that is part of the target entity.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(state = "stable")]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct EntityPath {
    pub value: rerun::datatypes::EntityPath,
}
