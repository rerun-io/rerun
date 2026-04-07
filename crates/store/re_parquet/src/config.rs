//! Configuration types for parquet loading.

use re_chunk::EntityPath;
use re_sdk_types::ComponentDescriptor;

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
    Prefix { delimiter: char },
}

impl Default for ColumnGrouping {
    fn default() -> Self {
        Self::Prefix { delimiter: '_' }
    }
}

// TODO(parquet): Will be replaced by lenses in py-chunk. Remove once available.
/// Rule for combining columns with matching suffixes into a typed archetype component.
///
/// When a set of columns whose names end with the specified `suffixes` (in order)
/// share a common prefix, they are combined into the target Rerun component.
///
/// Experimental: this API may change or be removed.
#[derive(Debug, Clone)]
pub struct ComponentRule {
    /// Ordered suffixes that identify columns for this component
    /// (e.g., `["_pos_x", "_pos_y", "_pos_z"]`).
    pub suffixes: Vec<String>,

    /// The Rerun component to construct from the matched columns.
    pub target: MappedComponent,
}

// TODO(parquet): Will be replaced by lenses in py-chunk. Remove once available.
/// A Rerun component that can be constructed from multiple scalar columns.
///
/// Experimental: this API may change or be removed.
#[derive(Debug, Clone, Copy)]
pub enum MappedComponent {
    /// 3 columns → `Translation3D` (`FixedSizeList(3, Float32)`).
    Translation3D,

    /// 4 columns → `RotationQuat` (`FixedSizeList(4, Float32)`).
    RotationQuat,
}

impl MappedComponent {
    pub(crate) fn descriptor(self) -> ComponentDescriptor {
        use re_sdk_types::archetypes::Transform3D;
        match self {
            Self::Translation3D => Transform3D::descriptor_translation(),
            Self::RotationQuat => Transform3D::descriptor_quaternion(),
        }
    }

    pub(crate) fn element_count(self) -> usize {
        match self {
            Self::Translation3D => 3,
            Self::RotationQuat => 4,
        }
    }
}

// TODO(parquet): Will be replaced by lenses in py-chunk. Remove once available.
/// Rule for combining columns with matching suffixes into named `Scalar` series.
///
/// Each group of matched columns becomes a multi-instance `Scalars` component
/// at the derived entity path, with a static `Name` component for the series
/// labels.
///
/// Experimental: this API may change or be removed.
#[derive(Debug, Clone)]
pub struct ScalarSuffixGroup {
    /// Ordered suffixes that identify columns (e.g., `["_x", "_y", "_z"]`).
    pub suffixes: Vec<String>,

    /// Display name for each series, in the same order as `suffixes`
    /// (e.g., `["x", "y", "z"]`).
    pub names: Vec<String>,
}

/// Configuration for parquet loading.
///
/// Fields marked "Experimental" are expected to change or be removed
/// as the parquet loading API matures. `column_grouping`, `index_columns`,
/// and `static_columns` are considered stable.
#[derive(Debug, Clone, Default)]
pub struct ParquetConfig {
    /// How to group columns into chunks.
    pub column_grouping: ColumnGrouping,

    /// Columns to use as timeline indices. When empty, a synthetic
    /// `row_index` sequence is generated automatically.
    pub index_columns: Vec<IndexColumn>,

    /// Column names with constant values — emitted as static data.
    pub static_columns: Vec<String>,

    // TODO(parquet): Ad-hoc; will be replaced by lenses in py-chunk.
    /// Experimental: suffix-to-archetype mapping rules.
    pub archetype_rules: Vec<ComponentRule>,

    // TODO(parquet): Ad-hoc; will be replaced by lenses in py-chunk.
    /// Experimental: suffix-to-Scalars grouping with named series.
    pub scalar_suffixes: Vec<ScalarSuffixGroup>,
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
