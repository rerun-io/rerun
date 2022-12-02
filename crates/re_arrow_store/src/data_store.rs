use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arrow2::array::{Array, Int64Vec, MutableArray, UInt64Vec};
use arrow2::datatypes::DataType;
use polars::prelude::{DataFrame, NamedFrom, Series};

use re_log_types::{ObjPath as EntityPath, Timeline};

use crate::TypedTimeInt;

// --- Task log ---
//
// High-level tasks that need work, ideally in some kind of decreasing importance order.
//
// General progress on the "make it" scale (status: in progress)
// - make it work (i.e. implemented) (status: in progress)
// - make it correct (i.e. _tested_!) (status: in progress)
// - make it fast (i.e. _benchmarked_!) (status: to do)

// Deduplication across timelines (status: should be done?)
//
// - deduplicate across timelines when inserting to multiple timelines in one call
// - does not cover deduplicating across timelines across different calls

// Standardize and put into writing insert-payload schema (status: to do)
//
// - should the entire payload be a list, for client-side batching?

// Index bucketing support (status: to do)
//
// - all index tables should be bucketized on a time range
// - the actual bucket-splitting is driven by size:
//    - the number of rows
//    - the size of the data (i.e. roughly nb_rows * nb_cols * sizeof(u64))
//
// - note: the bucket hierarchy is already there, it's just never splitted!

// Component bucketing support (status: to do)
//
// - all component tables should be bucketized on a row index range
// - the actual bucket-splitting is driven by the size of the actual data
//
// - note: the bucket hierarchy is already there, it's just never splitted!

// Support for non-numerical instance IDs
//
// - not sure where we stand on this

// Deletion endpoint (status: to do)
//
// - should just be a matter of inserting zeroes, the empty rows are already in there

// Serialization (status: to do)
//
// - support both serialization & deserialization, for .rrd

// Support for splats (status: to do)
//
// - many instances paired with a single-entry component = treat as splat

// Better resulting DataFrames for queries (status: to do)
//
// - component lists should probably be flattened at that point, behaving more like a table?
// - how should missing components be represented?

// First pass of correctness work (status: todo)
//
// - integration test suite for standard write, read & write+read paths
// - dedicated tests for all..
//    - ..features
//    - ..documented edge cases
//    - ..special paths due to optimization
//    - ..assertions
//    - ..errors & illegal state

// Performance pass for latest-at queries (status: to do)
//
// - get rid of useless clones in sort() code
// - optimize the per-component backwards search with per-component btrees?

// Range queries (status: to do)
//
// - have we actually settled on how we want these to behave precisely?

// First pass of performance work (status: to do)
//
// - put performance probes everywhere
// - provide helpers to gather detailed metrics: global, per-table, per-table-per-bucket
// - a lot of clones & copies need to go away
// - need integration benchmark suites
//    - write path, read path, write+read path

// Data purging (status: to do)
//
// - offer a way to drop both index & component buckets beyond a certain date

// Data store GUI browser (status: to do)
//
// - the store currently provides a very thorough Display implementation that makes it manageable
//   to keep track of what's going on internally
// - it'd be even better to have something similar but as an interactive UI panel, akin to
//   a SQL browser
// Inline / small-list optimization (status: to do)
//
// - if there is exactly ONE element in a component row, store it inline instead of a row index

// Schema registry / runtime payload validation (status: to do)
//
// - builtin components
// - opening the registration process to user-defined components.

// General deduplication (status: to do)
//
// - deduplicate across timelines across multiple calls
// - automagically deduplicate within a component table

// --- Data store ---

pub type ComponentName = String;
pub type ComponentNameRef<'a> = &'a str;

pub type RowIndex = u64;
pub type TypedTimeIntRange = std::ops::Range<TypedTimeInt>;

/// A complete data store: covers all timelines, all entities, everything.
///
/// `DataStore` provides a very thorough `Display` implementation that makes it manageable to
/// know what's going on internally.
/// For even more information, you can set `RERUN_DATA_STORE_DISPLAY_SCHEMAS=1` in your
/// environment, which will result in additional schema information being printed out.
#[derive(Default)]
pub struct DataStore {
    /// Maps an entity to its index, for a specific timeline.
    ///
    /// An index maps specific points in time to rows in component tables.
    //
    // TODO(cmc): needs a dedicated struct for the key instead of a tuple, so we don't have to
    // clone() everywhere.
    pub(crate) indices: HashMap<(Timeline, EntityPath), IndexTable>,
    /// Maps a component name to its associated table, for all timelines and all entities.
    ///
    /// A component table holds all the values ever inserted for a given component.
    pub(crate) components: HashMap<ComponentName, ComponentTable>,
}

impl std::fmt::Display for DataStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            indices,
            components,
        } = self;

        f.write_str("DataStore {\n")?;

        {
            f.write_str(&indent::indent_all_by(4, "indices: [\n"))?;
            for (_, index) in indices {
                f.write_str(&indent::indent_all_by(8, "IndexTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, index.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        {
            f.write_str(&indent::indent_all_by(4, "components: [\n"))?;
            for (_, comp) in components {
                f.write_str(&indent::indent_all_by(8, "ComponentTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, comp.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        f.write_str("}")?;

        Ok(())
    }
}

// --- Indices ---

/// An `IndexTable` maps specific points in time to rows in component tables.
///
/// Example of a time-based index table:
/// ```text
/// IndexTable {
///     timeline: log_time
///     entity: this/that
///     buckets: [
///         IndexBucket {
///             time range: from -∞ (inclusive) to +9223372036.855s (exlusive)
///             data (sorted=true): shape: (4, 4)
///             ┌──────────────────┬───────────┬───────────┬───────┐
///             │ time             ┆ positions ┆ instances ┆ rects │
///             │ ---              ┆ ---       ┆ ---       ┆ ---   │
///             │ str              ┆ u64       ┆ u64       ┆ u64   │
///             ╞══════════════════╪═══════════╪═══════════╪═══════╡
///             │ 10:03:24.825158Z ┆ null      ┆ null      ┆ 1     │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ 10:03:24.865158Z ┆ null      ┆ 1         ┆ 2     │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ 10:03:24.835158Z ┆ 1         ┆ null      ┆ null  │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ 10:03:24.845158Z ┆ null      ┆ 2         ┆ 3     │
///             └──────────────────┴───────────┴───────────┴───────┘
///
///         }
///     ]
/// }
/// ```
///
/// Example of a sequence-based index table:
/// ```text
/// IndexTable {
///     timeline: frame_nr
///     entity: this/that
///     buckets: [
///         IndexBucket {
///             time range: from -∞ (inclusive) to #9223372036854775807 (exlusive)
///             data (sorted=true): shape: (4, 4)
///             ┌──────┬───────────┬───────┬───────────┐
///             │ time ┆ positions ┆ rects ┆ instances │
///             │ ---  ┆ ---       ┆ ---   ┆ ---       │
///             │ str  ┆ u64       ┆ u64   ┆ u64       │
///             ╞══════╪═══════════╪═══════╪═══════════╡
///             │ #41  ┆ null      ┆ 2     ┆ 1         │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ #42  ┆ null      ┆ 3     ┆ 2         │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ #42  ┆ 1         ┆ null  ┆ null      │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ #43  ┆ null      ┆ 1     ┆ null      │
///             └──────┴───────────┴───────┴───────────┘
///
///         }
///     ]
/// }
/// ```
///
/// See also: [`Self::IndexBucket`].
#[derive(Debug)]
pub struct IndexTable {
    /// The timeline this table operates in, for debugging purposes.
    pub(crate) timeline: Timeline,
    /// The entity this table is related to, for debugging purposes.
    pub(crate) ent_path: EntityPath,

    /// The actual buckets, where the indices are stored.
    pub(crate) buckets: BTreeMap<TypedTimeInt, IndexBucket>,
}

impl std::fmt::Display for IndexTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            timeline,
            ent_path,
            buckets,
        } = self;

        f.write_fmt(format_args!("timeline: {}\n", timeline.name()))?;
        f.write_fmt(format_args!("entity: {}\n", ent_path))?;

        f.write_str("buckets: [\n")?;
        for (_, bucket) in buckets {
            f.write_str(&indent::indent_all_by(4, "IndexBucket {\n"))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string() + "\n"))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

/// An `IndexBucket` holds a size-delimited (data size and/or number of rows) chunk of a
/// [`Self::IndexTable`].
///
/// - The data size limit is for garbage collection purposes.
/// - The number of rows limit is to bound sorting costs on the read path.
///
/// See [`Self::IndexTable`] to get an idea of what an `IndexBucket` looks like in practice.
#[derive(Debug)]
pub struct IndexBucket {
    /// The time range covered by this bucket.
    pub(crate) time_range: TypedTimeIntRange,

    /// Whether the indices (all of them!) are currently sorted.
    ///
    /// Querying an `IndexBucket` will always trigger a sort if the indices aren't already sorted.
    pub(crate) is_sorted: bool,

    // The primary time index, which is guaranteed to be dense, and "drives" all other indices.
    //
    // All secondary indices are guaranteed to follow the same sort order and be the same length.
    pub(crate) times: Int64Vec,

    /// All secondary indices for this bucket (i.e. everything but time).
    ///
    /// One index per component: new components (and as such, new indices) can be added at any
    /// time!
    /// When that happens, they will be retro-filled with nulls so that they share the same
    /// length as the primary index.
    pub(crate) indices: HashMap<ComponentName, UInt64Vec>,
}

impl std::fmt::Display for IndexBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            time_range,
            is_sorted,
            times,
            indices,
        } = self;

        f.write_fmt(format_args!(
            "time range: from {} (inclusive) to {} (exlusive)\n",
            time_range.start, time_range.end,
        ))?;

        let typ = self.time_range.start.typ();
        let times = Series::new(
            "time",
            times
                .values()
                .iter()
                .map(|time| TypedTimeInt::from((typ, *time)).to_string())
                .collect::<Vec<_>>(),
        );

        let series = std::iter::once(times)
            .chain(indices.into_iter().map(|(name, index)| {
                let index = index
                    .values()
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| index.is_valid(i).then_some(*v))
                    .collect::<Vec<_>>();
                Series::new(name.as_str(), index)
            }))
            .collect::<Vec<_>>();
        let df = DataFrame::new(series).unwrap();
        f.write_fmt(format_args!("data (sorted={is_sorted}): {df:?}\n"))?;

        Ok(())
    }
}

// --- Components ---

/// A `ComponentTable` holds all the values ever inserted for a given component (provided they
/// are still alive, i.e. not GC'd).
///
/// Example of a component table holding instance IDs:
/// ```text
/// ComponentTable {
///     name: instances
///     buckets: [
///         ComponentBucket {
///             row offset: 0
///             time ranges:
///                 - frame_nr: from #41 (inclusive) to #43 (exlusive)
///                 - log_time: from 10:24:21.735485Z (inclusive) to 10:24:21.755485Z (exlusive)
///             data: shape: (3, 1)
///             ┌─────────────────────────────────────┐
///             │ instances                           │
///             │ ---                                 │
///             │ list[u32]                           │
///             ╞═════════════════════════════════════╡
///             │ []                                  │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
///             │ [478150623, 125728625, 4153899129]  │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
///             │ [1827991721, 3089121314, 427290248] │
///             └─────────────────────────────────────┘
///
///         }
///     ]
/// }
/// ```
///
/// Example of a component-table holding 2D positions:
///
/// ```text
/// ComponentTable {
///     name: positions
///     buckets: [
///         ComponentBucket {
///             row offset: 0
///             time ranges:
///                 - log_time: from 10:24:21.725485Z (inclusive) to 10:24:21.725485Z (exlusive)
///                 - frame_nr: from #42 (inclusive) to #43 (exlusive)
///             data: shape: (2, 1)
///             ┌────────────────────────────────────────────────────────────────┐
///             │ positions                                                      │
///             │ ---                                                            │
///             │ list[struct[2]]                                                │
///             ╞════════════════════════════════════════════════════════════════╡
///             │ []                                                             │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
///             │ [{6.172664,8.383976}, {2.059066,8.037471}, {0.42883,1.250902}] │
///             └────────────────────────────────────────────────────────────────┘
///
///         }
///     ]
/// }
/// ```
#[derive(Debug)]
pub struct ComponentTable {
    /// The component's name that this table is related to, for debugging purposes.
    pub(crate) name: Arc<String>,
    /// The component's datatype that this table is related to, for debugging purposes.
    pub(crate) datatype: DataType,

    /// The actual buckets, where the component data is stored.
    pub(crate) buckets: BTreeMap<RowIndex, ComponentBucket>,
}

impl std::fmt::Display for ComponentTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            datatype,
            buckets,
        } = self;

        f.write_fmt(format_args!("name: {}\n", name))?;
        if let Ok(v) = std::env::var("RERUN_DATA_STORE_DISPLAY_SCHEMAS") {
            if v == "1" {
                f.write_fmt(format_args!("datatype: {:#?}\n", datatype))?;
            }
        }

        f.write_str("buckets: [\n")?;
        for (_, bucket) in buckets {
            f.write_str(&indent::indent_all_by(4, "ComponentBucket {\n"))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string() + "\n"))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

/// A `ComponentBucket` holds a size-delimited (data size) chunk of a [`Self::ComponentTable`].
#[derive(Debug)]
pub struct ComponentBucket {
    /// The component's name, for debugging purposes.
    pub(crate) name: Arc<String>,

    /// The time ranges (plural!) covered by this bucket.
    /// Buckets are never sorted over time, so these time ranges can grow arbitrarily large.
    ///
    /// These are only used for garbage collection.
    pub(crate) time_ranges: HashMap<Timeline, TypedTimeIntRange>, // TODO: timetype

    /// What's the offset of that bucket in the shared table?
    pub(crate) row_offset: RowIndex,

    /// All the data for this bucket. This is a single column!
    // TODO: MutableArray!
    pub(crate) data: Box<dyn Array>,
}

impl std::fmt::Display for ComponentBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            time_ranges,
            row_offset,
            data,
        } = self;

        f.write_fmt(format_args!("row offset: {}\n", row_offset))?;

        f.write_str("time ranges:\n")?;
        for (timeline, time_range) in time_ranges {
            f.write_fmt(format_args!(
                "    - {}: from {} (inclusive) to {} (exlusive)\n",
                timeline.name(),
                time_range.start,
                time_range.end,
            ))?;
        }

        // TODO: I'm sure there's no need to clone here
        let series = Series::try_from((name.as_str(), data.clone())).unwrap();
        let df = DataFrame::new(vec![series]).unwrap();
        f.write_fmt(format_args!("data: {df:?}\n"))?;

        Ok(())
    }
}
