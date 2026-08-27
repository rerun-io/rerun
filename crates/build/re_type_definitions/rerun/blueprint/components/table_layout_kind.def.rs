// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How table records are presented.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub enum TableLayoutKind {
    /// Display records using [`rerun::blueprint::archetypes::TableLayout`].
    #[default]
    Table = 1,

    /// Display records using [`rerun::blueprint::archetypes::CardLayout`].
    Cards = 2,
}
