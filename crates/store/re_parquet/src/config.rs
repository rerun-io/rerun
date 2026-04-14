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

/// What to produce from a group of matched columns.
/// Highly experimental and will definitely change as
/// we add tools to support this more generically
#[derive(Debug, Clone)]
pub enum ColumnMapping {
    /// N columns → a Rerun component. Interleaved into `FixedSizeList(N, Float32)`.
    Component {
        /// Archetype + component descriptor used for the output chunk.
        descriptor: ComponentDescriptor,
    },

    /// N columns → multi-instance Scalars with named series.
    /// Interleaved into `FixedSizeList(N, Float64)` + companion names field.
    Scalars {
        /// Display name for each series, in the same order as `suffixes`.
        names: Vec<String>,
    },

    /// Translation + rotation columns → a `Transform3D` archetype.
    ///
    /// The translation suffixes come from the parent [`ColumnRule::suffixes`] field.
    /// When both suffix sets match with the same sub-prefix, the columns are
    /// combined into a `Transform3D` with translation and quaternion components.
    ///
    /// In struct mode this produces a nested struct with `translation` and
    /// `quaternion` fields. In flat mode, two components at the same entity path.
    Transform {
        /// Ordered suffixes that identify the rotation columns
        /// (e.g., `["_quat_x", "_quat_y", "_quat_z", "_quat_w"]`).
        rotation_suffixes: Vec<String>,
    },
}

impl ColumnMapping {
    /// `Translation3D` component mapping.
    pub fn translation3d() -> Self {
        use re_sdk_types::archetypes::Transform3D;
        Self::Component {
            descriptor: Transform3D::descriptor_translation(),
        }
    }

    /// `RotationQuat` component mapping.
    pub fn rotation_quat() -> Self {
        use re_sdk_types::archetypes::Transform3D;
        Self::Component {
            descriptor: Transform3D::descriptor_quaternion(),
        }
    }

    /// `RotationAxisAngle` component mapping.
    pub fn rotation_axis_angle() -> Self {
        use re_sdk_types::archetypes::Transform3D;
        Self::Component {
            descriptor: Transform3D::descriptor_rotation_axis_angle(),
        }
    }

    /// `Scale3D` component mapping.
    pub fn scale3d() -> Self {
        use re_sdk_types::archetypes::Transform3D;
        Self::Component {
            descriptor: Transform3D::descriptor_scale(),
        }
    }

    /// `Transform3D` mapping (translation + rotation quaternion).
    pub fn transform(rotation_suffixes: Vec<String>) -> Self {
        Self::Transform { rotation_suffixes }
    }
}

/// Rule for combining columns with matching suffixes into a typed component.
///
/// When a set of columns whose names end with the specified `suffixes` (in order)
/// share a common prefix, they are combined according to `mapping`.
///
/// Rules are processed in list order; the first rule whose suffixes match a set
/// of columns wins. Put specific rules before broad catch-all rules.
///
/// Experimental: this API may change or be removed.
#[derive(Debug, Clone)]
pub struct ColumnRule {
    /// Ordered suffixes that identify columns (e.g., `["_pos_x", "_pos_y", "_pos_z"]`).
    pub suffixes: Vec<String>,

    /// What to produce from the matched columns.
    pub mapping: ColumnMapping,

    /// Optional override appended to the sub-prefix to form the struct field name.
    ///
    /// When present and `sub_prefix` is non-empty: `field_name = "{sub_prefix}{override}"`.
    /// When present and `sub_prefix` is empty: `field_name = override` (leading `_` stripped).
    /// The `suffix_fallback` is ignored when override is set.
    pub field_name_override: Option<String>,
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
    /// Experimental: suffix-based column combination rules.
    pub column_rules: Vec<ColumnRule>,
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
