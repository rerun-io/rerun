// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How table column names are displayed when no explicit display name is configured.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub enum ColumnDisplayMode {
    /// Humanized field name, e.g. `Start time`, with `rerun_` prefixes removed.
    ///
    /// Colliding names are expanded with humanized parent segments across all columns,
    /// including hidden columns, until distinct or fully expanded.
    #[default]
    Compact = 1,

    /// Component name, e.g. `RecordingInfo:start_time`.
    ///
    /// Colliding names are expanded with physical parent segments across all columns,
    /// including hidden columns, until distinct or fully expanded.
    Component = 2,

    /// Full physical name, e.g. `property:RecordingInfo:start_time`.
    Full = 3,
}
