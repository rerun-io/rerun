// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The column and direction a table's rows are sorted by.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, Hash))]
#[rerun(state = "unstable")]
pub struct SortBy {
    /// The name of the column the rows are sorted by.
    ///
    /// Same semantics as [`rerun::blueprint::components::ColumnName`].
    pub column: rerun::encodings::Utf8,

    /// The direction the rows are sorted in.
    ///
    /// If not set, the viewer sorts ascending.
    pub direction: Option<rerun::blueprint::encodings::SortDirection>,
}

/// The order rows are sorted in.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
#[rust(arrow_opt)]
pub enum SortDirection {
    /// Smallest values first.
    #[default]
    Ascending = 1,

    /// Largest values first.
    Descending = 2,
}
