// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The visual appearance of a point in e.g. a 2D plot.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum MarkerShape {
    /// `⏺`
    #[default]
    Circle = 1,

    /// `◆`
    Diamond = 2,

    /// `◼️`
    Square = 3,

    /// `x`
    Cross = 4,

    /// `+`
    Plus = 5,

    /// `▲`
    Up = 6,

    /// `▼`
    Down = 7,

    /// `◀`
    Left = 8,

    /// `▶`
    Right = 9,

    /// `*`
    Asterisk = 10,
}
