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

/// The complete data store: covers all timelines, all entities, everything.
#[derive(Default)]
pub struct DataStore {
    /// Maps an entity to its index, for a specific timeline.
    // TODO: needs a dedicated struct for the key, so we don't have to clone() everywhere.
    pub(crate) indices: HashMap<(Timeline, EntityPath), IndexTable>,
    /// Maps a component to its data, for all timelines and all entities.
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

/// A chunked index, bucketized over time and space (whichever comes first).
///
/// Each bucket covers a half-open time range.
/// These time ranges are guaranteed to be non-overlapping.
///
/// ```text
/// Bucket #1: #202..#206
///
/// time | instances | comp#1 | comp#2 | … | comp#N |
/// ---------------------------------------|--------|
/// #202 | 2         | 2      | -      | … | 1      |
/// #203 | 3         | -      | 3      | … | 4      |
/// #204 | 4         | 6      | -      | … | -      |
/// #204 | 4         | 8      | 8      | … | -      |
/// #205 | 0         | 0      | 0      | … | -      |
/// #205 | 5         | -      | 9      | … | 2      |
/// ```
///
/// TODO:
/// - talk about out of order data and the effect it has
/// - talk about deletion
/// - talk about _lack of_ smallvec optimization
/// - talk (and test) append-only behavior
///
/// See also: [`Self::IndexBucket`].
//
//
// Each entry is a row index. It's nullable, with `null` = no entry.
#[derive(Debug)]
pub struct IndexTable {
    pub(crate) timeline: Timeline,
    pub(crate) ent_path: EntityPath,
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

/// TODO
//
// Has a max size of 128MB OR 10k rows, whatever comes first.
// The size-limit is so we can purge memory in small buckets
// The row-limit is to avoid slow re-sorting at query-time
#[derive(Debug)]
pub struct IndexBucket {
    /// The time range covered by this bucket.
    pub(crate) time_range: TypedTimeIntRange,

    /// Whether the indices are currently sorted.
    ///
    /// Querying an `IndexBucket` will always trigger a sort if the indices aren't already sorted.
    pub(crate) is_sorted: bool,

    /// All indices for this bucket.
    ///
    /// Each column in this dataframe corresponds to a component.
    //
    // new columns may be added at any time
    // sorted by the first column, time (if [`Self::is_sorted`])
    //
    // TODO: some components are always present: timelines, instances
    pub(crate) times: Int64Vec,
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

/// A chunked component table (i.e. a single column), bucketized by size only.
//
// The ComponentTable maps a row index to a list of values (e.g. a list of colors).
#[derive(Debug)]
pub struct ComponentTable {
    /// The component's name.
    pub(crate) name: Arc<String>,
    /// The component's datatype.
    pub(crate) datatype: DataType,
    /// Each bucket covers an arbitrary range of rows.
    /// How large that range is will depend on the size of the actual data, which is the actual
    /// trigger for chunking.
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
        // TODO: doc
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

/// TODO
//
// Has a max-size of 128MB or so.
// We bucket the component table so we can purge older parts when needed.
#[derive(Debug)]
pub struct ComponentBucket {
    /// The component's name.
    pub(crate) name: Arc<String>,

    /// The time ranges (plural!) covered by this bucket.
    ///
    /// Buckets are never sorted over time, time ranges can grow arbitrarily large.
    //
    // Used when to figure out if we can purge it.
    // Out-of-order inserts can create huge time ranges here,
    // making some buckets impossible to purge, but we accept that risk.
    //
    // TODO: this is for purging only
    pub(crate) time_ranges: HashMap<Timeline, TypedTimeIntRange>, // TODO: timetype

    // TODO
    pub(crate) row_offset: RowIndex,

    /// All the data for this bucket. This is a single column!
    ///
    /// Each row contains the data for all instances.
    /// Instances within a row are sorted
    //
    // maps a row index to a list of values (e.g. a list of colors).
    //
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
