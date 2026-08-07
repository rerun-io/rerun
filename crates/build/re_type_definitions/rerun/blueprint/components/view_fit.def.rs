// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Determines whether an image or texture should be scaled to fit the viewport.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "stable")]
pub enum ViewFit {
    /// No scaling, pixel size will match the image's width/height dimensions in pixels.
    Original = 1,

    /// Scale the image for the largest possible fit in the view's container.
    Fill = 2,

    /// Scale the image for the largest possible fit in the view's container, but keep the original aspect ratio.
    #[default]
    FillKeepAspectRatio = 3,
}
