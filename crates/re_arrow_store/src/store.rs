use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arrow2::array::{Array, Int64Vec, UInt64Vec};
use arrow2::datatypes::DataType;

use re_log_types::{
    ComponentName, ObjPath as EntityPath, ObjPathHash as EntityPathHash, TimeInt, TimeRange,
    Timeline,
};

// --- Data store ---

pub type RowIndex = u64;

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
    pub(crate) indices: HashMap<(Timeline, EntityPathHash), IndexTable>,

    /// Maps a component name to its associated table, for all timelines and all entities.
    ///
    /// A component table holds all the values ever inserted for a given component.
    pub(crate) components: HashMap<ComponentName, ComponentTable>,
}

impl std::fmt::Display for DataStore {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            indices,
            components,
        } = self;

        f.write_str("DataStore {\n")?;

        {
            f.write_str(&indent::indent_all_by(4, "indices: [\n"))?;
            for index in indices.values() {
                f.write_str(&indent::indent_all_by(8, "IndexTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, index.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        {
            f.write_str(&indent::indent_all_by(4, "components: [\n"))?;
            for comp in components.values() {
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
///             ┌──────────────────┬───────────┬───────┬───────────┐
///             │ time             ┆ positions ┆ rects ┆ instances │
///             │ ---              ┆ ---       ┆ ---   ┆ ---       │
///             │ str              ┆ u64       ┆ u64   ┆ u64       │
///             ╞══════════════════╪═══════════╪═══════╪═══════════╡
///             │ 18:04:35.284851Z ┆ null      ┆ 1     ┆ null      │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ 18:04:35.284851Z ┆ 1         ┆ null  ┆ null      │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ 18:04:35.294851Z ┆ null      ┆ 3     ┆ 2         │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ 18:04:35.304851Z ┆ null      ┆ 2     ┆ 1         │
///             └──────────────────┴───────────┴───────┴───────────┘
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
///             ┌──────┬───────────┬───────────┬───────┐
///             │ time ┆ instances ┆ positions ┆ rects │
///             │ ---  ┆ ---       ┆ ---       ┆ ---   │
///             │ str  ┆ u64       ┆ u64       ┆ u64   │
///             ╞══════╪═══════════╪═══════════╪═══════╡
///             │ #41  ┆ 1         ┆ null      ┆ 2     │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ #42  ┆ 2         ┆ null      ┆ 3     │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ #42  ┆ null      ┆ 1         ┆ null  │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ #43  ┆ null      ┆ null      ┆ 1     │
///             └──────┴───────────┴───────────┴───────┘
///         }
///     ]
/// }
/// ```
///
/// See also: [`IndexBucket`].
#[derive(Debug)]
pub struct IndexTable {
    /// The timeline this table operates in, for debugging purposes.
    pub(crate) timeline: Timeline,
    /// The entity this table is related to, for debugging purposes.
    pub(crate) ent_path: EntityPath,

    /// The actual buckets, where the indices are stored.
    pub(crate) buckets: BTreeMap<TimeInt, IndexBucket>,
}

impl std::fmt::Display for IndexTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            timeline,
            ent_path,
            buckets,
        } = self;

        f.write_fmt(format_args!("timeline: {}\n", timeline.name()))?;
        f.write_fmt(format_args!("entity: {}\n", ent_path))?;

        f.write_str("buckets: [\n")?;
        for bucket in buckets.values() {
            f.write_str(&indent::indent_all_by(4, "IndexBucket {\n"))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string() + "\n"))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

/// An `IndexBucket` holds a size-delimited (data size and/or number of rows) chunk of a
/// [`IndexTable`].
///
/// - The data size limit is for garbage collection purposes.
/// - The number of rows limit is to bound sorting costs on the read path.
///
/// See [`IndexTable`] to get an idea of what an `IndexBucket` looks like in practice.
#[derive(Debug)]
pub struct IndexBucket {
    /// The timeline the bucket's parent table operates in, for debugging purposes.
    pub(crate) timeline: Timeline,

    /// The time range covered by this bucket.
    pub(crate) time_range: TimeRange,

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
            timeline,
            time_range,
            is_sorted,
            times,
            indices,
        } = self;

        f.write_fmt(format_args!(
            "time range: from {} to {} (all inclusive)\n",
            timeline.typ().format(time_range.min),
            timeline.typ().format(time_range.max),
        ))?;

        #[cfg(not(target_arch = "wasm32"))]
        {
            use arrow2::array::MutableArray as _;
            use polars::prelude::{DataFrame, NamedFrom as _, Series};

            let typ = timeline.typ();
            let times = Series::new(
                "time",
                times
                    .values()
                    .iter()
                    .map(|time| typ.format(TimeInt::from(*time)))
                    .collect::<Vec<_>>(),
            );

            let series = std::iter::once(times)
                .chain(indices.iter().map(|(name, index)| {
                    let index = index
                        .values()
                        .iter()
                        .enumerate()
                        .map(|(i, v)| index.is_valid(i).then_some(*v))
                        .collect::<Vec<_>>();
                    Series::new(name.as_str(), index)
                }))
                .collect::<Vec<_>>();
            let df = DataFrame::new(series).unwrap();
            f.write_fmt(format_args!("data (sorted={is_sorted}): {df:?}\n"))?;
        }

        #[cfg(target_arch = "wasm32")]
        {
            _ = time_range;
            _ = is_sorted;
            _ = times;
            _ = indices;
        }

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
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            datatype,
            buckets,
        } = self;

        f.write_fmt(format_args!("name: {}\n", name))?;
        if matches!(
            std::env::var("RERUN_DATA_STORE_DISPLAY_SCHEMAS").as_deref(),
            Ok("1")
        ) {
            f.write_fmt(format_args!("datatype: {:#?}\n", datatype))?;
        }

        f.write_str("buckets: [\n")?;
        for bucket in buckets.values() {
            f.write_str(&indent::indent_all_by(4, "ComponentBucket {\n"))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string() + "\n"))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

/// A `ComponentBucket` holds a size-delimited (data size) chunk of a [`ComponentTable`].
#[derive(Debug)]
pub struct ComponentBucket {
    /// The component's name, for debugging purposes.
    pub(crate) name: Arc<String>,

    /// The time ranges (plural!) covered by this bucket.
    /// Buckets are never sorted over time, so these time ranges can grow arbitrarily large.
    ///
    /// These are only used for garbage collection.
    pub(crate) time_ranges: HashMap<Timeline, TimeRange>,

    /// What's the offset of that bucket in the shared table?
    pub(crate) row_offset: RowIndex,

    /// All the data for this bucket. This is a single column!
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
                "    - {}: from {} to {} (all inclusive)\n",
                timeline.name(),
                timeline.typ().format(time_range.min),
                timeline.typ().format(time_range.max),
            ))?;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use polars::prelude::{DataFrame, Series};
            // TODO(cmc): I'm sure there's no need to clone here
            let series = Series::try_from((name.as_str(), data.clone())).unwrap();
            let df = DataFrame::new(vec![series]).unwrap();
            f.write_fmt(format_args!("data: {df:?}\n"))?;
        }

        #[cfg(target_arch = "wasm32")]
        {
            _ = name;
            _ = time_ranges;
            _ = row_offset;
            _ = data;
        }

        Ok(())
    }
}
