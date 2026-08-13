// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// One of four 2D corners, typically used to align objects.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rerun(state = "stable")]
pub enum Corner2D {
    /// Left top corner.
    LeftTop = 1,

    /// Right top corner.
    RightTop = 2,

    /// Left bottom corner.
    #[default]
    LeftBottom = 3,

    /// Right bottom corner.
    RightBottom = 4,
}
