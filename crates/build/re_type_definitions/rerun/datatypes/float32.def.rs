// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A single-precision 32-bit IEEE 754 floating point number.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "float")]
#[python(
    array_aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[Sequence[float]] | Sequence[float]"
)]
#[rust(derive(
    Default,
    Copy,
    PartialEq,
    PartialOrd,
    bytemuck::Pod,
    bytemuck::Zeroable
))]
#[rust(override_crate = "re_types_core")]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct Float32 {
    pub value: f32,
}
