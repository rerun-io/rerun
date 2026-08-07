// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A string identifier for a transform frame.
///
/// Transform frames may be derived from entity paths to refer to Rerun's implicit
/// entity path driven hierarchy which is defined via [archetypes.Transform3D], [archetypes.Pinhole] etc..
/// These implicit transform frames look like `tf#path/to/entity`.
///
/// Note that any [archetypes.Transform3D]s logged with both `parent_frame` and `child_frame` set
/// describes a relationship between these parent and child transform frames, **not** the transform frame
/// that the entity path may be using (defined by an [archetypes.CoordinateFrame]).
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str, Sequence[str]")]
#[rerun(state = "stable")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct TransformFrameId {
    pub value: rerun::datatypes::Utf8,
}
