//! Configuration types for parquet loading.

use re_chunk::EntityPath;

/// Strategy for grouping parquet columns into Rerun chunks.
///
/// Grouping reduces RRD size by sharing timeline data and row IDs across
/// columns in the same chunk instead of duplicating them per column.
#[derive(Debug, Clone)]
pub enum ColumnGrouping {
    /// Each column becomes its own entity/chunk (no deduplication).
    Individual,

    /// Group columns that share a common prefix before `delimiter`.
    ///
    /// For example, with `delimiter: '_'`, columns `camera_rgb` and
    /// `camera_depth` are grouped under entity `/camera` with components
    /// `rgb` and `depth`. Columns without the delimiter are placed in
    /// their own single-column group.
    Prefix { delimiter: char, use_structs: bool },

    /// Group columns by explicit prefix strings.
    ///
    /// Each column is checked against the prefixes in longest-first order.
    /// The first matching prefix is stripped, and the column is added to that
    /// prefix's group. One leading underscore is also stripped from the
    /// remainder (so prefix `"cat"` on column `"cat_foo"` gives component `"foo"`).
    ///
    /// Columns that don't match any prefix become individual groups.
    ///
    /// **Note:** Matching uses simple `str::starts_with`, not delimiter-aware
    /// boundaries. Prefix `"cat"` will match column `"catdog"` (remainder
    /// `"dog"`). To avoid unintended matches, choose prefixes that are
    /// unambiguous in your column namespace, or include the delimiter in the
    /// prefix (e.g., `"cat_"` — though the leading-underscore strip then
    /// becomes a no-op since there is no underscore to strip).
    ExplicitPrefixes {
        prefixes: Vec<String>,
        use_structs: bool,
    },
}

impl Default for ColumnGrouping {
    fn default() -> Self {
        Self::Prefix {
            delimiter: '_',
            use_structs: true,
        }
    }
}

/// Configuration for parquet loading.
#[derive(Debug, Clone, Default)]
pub struct ParquetConfig {
    /// How to group columns into chunks.
    pub column_grouping: ColumnGrouping,

    /// Columns to use as timeline indices. When empty, a synthetic
    /// `row_index` sequence is generated automatically.
    pub index_columns: Vec<IndexColumn>,

    /// Column names with constant values — emitted as static data.
    pub static_columns: Vec<String>,
}

impl ParquetConfig {
    /// Default entity path prefix used when none is specified by the caller.
    pub fn default_entity_path_prefix() -> EntityPath {
        EntityPath::from("/")
    }
}

/// Specifies how a parquet column maps to a Rerun timeline.
#[derive(Debug, Clone)]
pub struct IndexColumn {
    /// Column name in the parquet file.
    pub name: String,

    /// What kind of timeline this represents.
    pub index_type: IndexType,
}

/// The type and scale of an index column.
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
