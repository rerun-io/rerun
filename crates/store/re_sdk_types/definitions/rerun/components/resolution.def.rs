// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Pixel resolution width & height, e.g. of a camera sensor.
///
/// Typically in integer units, but for some use cases floating point may be used.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq))]
#[rerun(state = "stable")]
pub struct Resolution {
    pub resolution: rerun::datatypes::Vec2D,
}
