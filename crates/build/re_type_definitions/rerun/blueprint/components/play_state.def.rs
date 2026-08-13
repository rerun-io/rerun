// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The current play state.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub enum PlayState {
    /// Time doesn't move.
    Paused = 1,

    /// Time move steadily.
    #[default]
    Playing = 2,

    /// Follow the latest available data.
    Following = 3,
}
