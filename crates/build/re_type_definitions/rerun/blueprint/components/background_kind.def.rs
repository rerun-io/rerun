// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The type of the background in a view.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "stable")]
pub enum BackgroundKind {
    /// A dark gradient.
    ///
    /// In 3D views it changes depending on the direction of the view.
    #[default]
    GradientDark = 1,

    /// A bright gradient.
    ///
    /// In 3D views it changes depending on the direction of the view.
    GradientBright = 2,

    /// Simple uniform color.
    SolidColor = 3,
}
