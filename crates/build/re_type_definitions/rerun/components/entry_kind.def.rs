// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The kind of a catalog entry.
///
/// The values match those of `rerun.cloud.v1alpha1.EntryKind`.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "unstable")]
pub enum EntryKind {
    /// A dataset.
    Dataset = 1,

    /// A view over a dataset.
    DatasetView = 2,

    /// A table.
    Table = 3,

    /// A view over a table.
    TableView = 4,

    /// A dataset holding the blueprints of another entry.
    BlueprintDataset = 5,

    /// A dataset holding the static assets shared by the segments of another dataset.
    AssetDataset = 6,
}
