// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The metric size of one grid cell in local scene units.
///
/// E.g. for 2D grid maps, this is the physical size represented by a single pixel or cell.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float32]")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct CellSize {
    pub value: rerun::datatypes::Float32,
}
