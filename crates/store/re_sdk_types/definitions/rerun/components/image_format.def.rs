// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The metadata describing the contents of a [components.ImageBuffer].
#[rerun::rerun_type]
#[rust(derive(Default, Copy, PartialEq, Eq, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct ImageFormat {
    pub image_format: rerun::datatypes::ImageFormat,
}
