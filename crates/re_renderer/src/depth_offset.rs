/// Depth offset used to resolve z-fighting between 2D primitives.
///
/// Zero means no offset.
/// Higher values push an object towards the viewer, lower away from the viewer.
/// Depth offsets are applied in the shader as-late as possible.
///
///
/// Implementation notes:
/// ---------------------------
/// `WebGPU` provides a [per-pipeline depth bias](https://www.w3.org/TR/webgpu/#abstract-opdef-biased-fragment-depth) which would be optimal for this.
/// However, this would require us to create a new pipeline for every new offset! Instead,
/// we do manual offsetting in the vertex shader. This is more error prone but very dynamic!
///
/// Shaders typically pass in a f32 for easy of use and speed (no unpacking or expensive integer math required)
/// This value is 16 bit to ensure that it is correctly represented in any case and allow packing if necessary.
pub type DepthOffset = i16;
