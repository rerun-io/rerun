//! Configuration types for HDF5 loading.

use re_chunk::EntityPath;

/// Configuration for HDF5 loading.
#[derive(Debug, Clone)]
pub struct Hdf5Config {
    /// The group to treat as the file root (`None` → the whole file).
    ///
    /// Everything is scoped to this subtree as if it were the root: only its
    /// datasets are loaded and aligned, and all other paths in this config —
    /// as well as the emitted entity paths — are interpreted relative to it.
    pub root_group: Option<String>,

    /// The single file-wide timeline index.
    ///
    /// `None` → synthesize a `row_index` sequence timeline instead.
    /// Interpreted relative to [`Self::root_group`].
    pub index_column: Option<IndexColumn>,

    /// Dataset or group paths to exclude entirely (a group path excludes its whole subtree).
    /// Interpreted relative to [`Self::root_group`].
    pub ignore_datasets: Vec<String>,

    /// Pack each group's row-aligned datasets into a single struct component (default)
    /// vs one component per dataset.
    pub use_structs: bool,

    /// Prefix prepended to every emitted entity path (defaults to the root `/`).
    pub entity_path_prefix: EntityPath,
}

impl Default for Hdf5Config {
    fn default() -> Self {
        Self {
            root_group: None,
            index_column: None,
            ignore_datasets: Vec::new(),
            use_structs: true,
            entity_path_prefix: EntityPath::from("/"),
        }
    }
}

/// Specifies which HDF5 dataset provides the file-wide timeline.
#[derive(Debug, Clone)]
pub struct IndexColumn {
    /// Full dataset path within the file, e.g. `/time`.
    pub path: String,

    /// What kind of timeline this represents.
    pub index_type: IndexType,
}

/// The type and scale of an index column.
//TODO(ab): the same exists in re_parquet. Consider reuse?
#[derive(Debug, Clone, Copy)]
pub enum IndexType {
    /// Timestamp (time since epoch). Raw values are scaled to nanoseconds.
    Timestamp(TimeUnit),

    /// Duration (elapsed time). Raw values are scaled to nanoseconds.
    Duration(TimeUnit),

    /// Ordinal sequence index. No scaling applied.
    Sequence,
}

impl IndexType {
    /// Multiplier to convert raw values to nanoseconds. Returns 1 for Sequence.
    pub(crate) fn ns_multiplier(self) -> i64 {
        match self {
            Self::Timestamp(unit) | Self::Duration(unit) => unit.ns_multiplier(),
            Self::Sequence => 1,
        }
    }
}

/// Scale of raw time values. Determines the multiplier to convert to nanoseconds.
#[derive(Debug, Clone, Copy, Default)]
pub enum TimeUnit {
    #[default]
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
}

impl TimeUnit {
    /// Multiplier to convert a raw value in this unit to nanoseconds.
    pub fn ns_multiplier(self) -> i64 {
        match self {
            Self::Nanoseconds => 1,
            Self::Microseconds => 1_000,
            Self::Milliseconds => 1_000_000,
            Self::Seconds => 1_000_000_000,
        }
    }
}
