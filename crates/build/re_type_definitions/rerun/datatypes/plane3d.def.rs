// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An infinite 3D plane represented by a unit normal vector and a distance.
///
/// Any point P on the plane fulfills the equation `dot(xyz, P) - d = 0`,
/// where `xyz` is the plane's normal and `d` the distance of the plane from the origin.
/// This representation is also known as the Hesse normal form.
///
/// Note: although the normal will be passed through to the
/// datastore as provided, when used in the Viewer, planes will always be normalized.
/// I.e. the plane with xyz = (2, 0, 0), d = 1 is equivalent to xyz = (1, 0, 0), d = 0.5
#[rerun::rerun_type]
#[arrow(transparent)]
#[cpp(no_field_ctors)]
#[python(array_aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[Sequence[float]]")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "C")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct Plane3D {
    pub xyzd: [f32; 4],
}
