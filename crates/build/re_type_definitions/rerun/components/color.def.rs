// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.
///
/// The color is stored as a 32-bit integer, where the most significant
/// byte is `R` and the least significant byte is `A`.
///
/// \py Float colors are assumed to be in 0-1 gamma sRGB space.
/// \py All other colors are assumed to be in 0-255 gamma sRGB space.
/// \py If there is an alpha, we assume it is in linear space, and separate (NOT pre-multiplied).
#[rerun::rerun_type]
#[python(aliases = "int | Sequence[int] | npt.NDArray[np.uint8 | np.float32 | np.float64]")]
#[python(
    array_aliases = "int | Sequence[Sequence[int]] | npt.NDArray[np.uint8 | np.uint32 | np.float32 | np.float64]"
)]
#[rust(derive(
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    bytemuck::Pod,
    bytemuck::Zeroable
))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Color {
    pub rgba: rerun::datatypes::Rgba32,
}
