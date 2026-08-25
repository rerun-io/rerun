// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How a table column value is rendered.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub enum TableCellKind {
    /// Infer the renderer from the component or Arrow datatype.
    #[default]
    Auto = 1,

    /// Render a Rerun URI as an interactive link.
    Link = 2,

    /// Render an image blob as a thumbnail.
    Thumbnail = 3,

    /// Render a boolean as a flag.
    ///
    /// Card layouts place the first flag field in the special flag position and
    /// currently support at most one flag field.
    Flag = 4,

    /// Render a recording reference as embedded views.
    ///
    /// The column must also have [`rerun::blueprint::archetypes::TableColumnPreview`] configuration with at least one view.
    /// All preview columns use the shared [`rerun::blueprint::archetypes::PreviewsConfig`].
    Preview = 5,

    /// Interpret scalar `i32` values as `rerun.cloud.v1alpha1.EntryKind` enum values and render their human-readable names.
    ///
    /// Unknown numeric values are rendered as `Unknown EntryKind`.
    EntryKind = 6,
}
