// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A Quaternion represented by 4 real numbers.
///
/// Note: although the x,y,z,w components of the quaternion will be passed through to the
/// datastore as provided, when used in the Viewer Quaternions will always be normalized.
#[rerun::rerun_type]
#[arrow(transparent)]
#[cpp(no_field_ctors)]
#[python(array_aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[Sequence[float]]")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "C")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct Quaternion {
    pub xyzw: [f32; 4],
}
