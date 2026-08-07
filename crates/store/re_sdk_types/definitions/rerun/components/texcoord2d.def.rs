// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 2D texture UV coordinate.
///
/// Texture coordinates specify a position on a 2D texture.
/// A range from 0-1 covers the entire texture in the respective dimension.
/// Unless configured otherwise, the texture repeats outside of this range.
/// Rerun uses top-left as the origin for UV coordinates.
///
///   0     U     1
/// 0 + --------- →
///   |           .
///   |           .
/// V |           .
///   |           .
/// 1 ↓ . . . . . .
///
/// This is the same convention as in Vulkan/Metal/DX12/WebGPU, but (!) unlike OpenGL,
/// which places the origin at the bottom-left.
#[rerun::rerun_type]
#[python(aliases = "npt.NDArray[np.float32] | Sequence[float] | Tuple[float, float]")]
#[python(array_aliases = "npt.NDArray[np.float32] | Sequence[float]")]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Texcoord2D {
    pub uv: rerun::datatypes::Vec2D,
}
