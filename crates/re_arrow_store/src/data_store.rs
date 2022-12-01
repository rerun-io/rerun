use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure};
use arrow2::array::{new_empty_array, Array, Int64Array, ListArray, PrimitiveArray, StructArray};
use arrow2::buffer::Buffer;
use arrow2::chunk::Chunk;
use arrow2::compute::concatenate::concatenate;
use arrow2::datatypes::{DataType, Schema};
use nohash_hasher::IntMap;
use polars::prelude::IndexOfSchema;

use re_log_types::arrow::{
    filter_time_cols, ENTITY_PATH_KEY, TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME,
};
use re_log_types::{ObjPath as EntityPath, TimeInt, TimeType, Timeline};

// TODO: going for the usual principles here:
// - be liberal in what you accept, be strict in what you return
// - 1) make it work 2) make it correct (i.e. _tested_) 3) make it fast

// TODO:
// - write path
// - read path
// - purge / GC (later)

// TODO:
// - keeping low level _for now_ (i.e. no polars at this layer)
//    - need to get familiar with what's actually going on under the good
//    - don't add layers until we have a use case for them

// TODO:
// - pretty debug dumps

// ---

// https://www.notion.so/rerunio/Arrow-Table-Design-cd77528c77ae4aa4a8c566e2ec29f84f

// TODO: perf probes
// TODO: every error and assert paths must be _TESTED_!!!

// TODO: recursive Display impls for everything
// TODO: recursive Iterator impls for everything

type ComponentName = String;
type ComponentNameRef<'a> = &'a str;
type RowIndex = u64;
type TimeIntRange = std::ops::Range<TimeInt>;

/// The complete data store: covers all timelines, all entities, everything.
#[derive(Default)]
pub struct DataStore {
    /// Maps an entity to its index, for a specific timeline.
    indices: HashMap<(Timeline, EntityPath), IndexTable>,
    /// Maps a component to its data, for all timelines and all entities.
    components: HashMap<ComponentName, ComponentTable>,
}

impl std::fmt::Display for DataStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            indices,
            components,
        } = self;

        // TODO: indices

        f.write_str("DataStore {\n")?;
        f.write_str(&indent::indent_all_by(4, "components: [\n"))?;
        for (_, comp) in components {
            f.write_str(&indent::indent_all_by(8, "ComponentTable {\n"))?;
            f.write_str(&indent::indent_all_by(12, comp.to_string() + "\n"))?;
            f.write_str(&indent::indent_all_by(8, "}\n"))?;
        }
        f.write_str(&indent::indent_all_by(4, "]\n"))?;
        f.write_str("}")?;

        Ok(())
    }
}

impl DataStore {
    //     fn insert_components(&mut self, timeline, time, obj_path,
    //         components: Map<ComponentName, ArrowStore>) {
    //         let instance_row = self.components["instance_keys"].push(instance_keys);
    //         let pos_row = self.components["positions"].push(positions);
    //         self.main_tables[(timeline, obj_path)]
    //             .insert(time, instance_row, pos_row);
    //     }
    pub fn insert(&mut self, schema: &Schema, msg: Chunk<Box<dyn Array>>) -> anyhow::Result<()> {
        // dbg!(&schema);
        // dbg!(&msg);

        // TODO: might make sense to turn the entire top-level message into a list, to help
        // with batching on the client side.

        let ent_path = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or_else(|| anyhow!("expect entity path in top-level message's metadata"))
            .map(|path| EntityPath::from(path.as_str()))?;

        let timelines = extract_timelines(schema, &msg)?;

        // TODO: Let's start the very dumb way: one bucket per TimeInt, then we'll deal with
        // actual ranges.
        for (timeline, time) in &timelines {
            let index = self
                .indices
                .entry((timeline.clone(), ent_path.clone()))
                .or_insert(Default::default());
        }

        let components = extract_components(schema, &msg)?;

        // let mut indices = HashMap::with_capacity(components.len());
        for (name, component) in components {
            let table = self.components.entry(name.to_owned()).or_insert_with(|| {
                ComponentTable::new(name.to_owned(), component.data_type().clone())
            });

            let row_idx = table.insert(&timelines, component);
            // dbg!(row_idx);
        }

        Ok(())
    }

    pub fn query() {}
}

// TODO: document the datamodel here: 1 timestamp per message per timeline.
// TODO: is that the right data model for this? is it optimal? etc
fn extract_timelines<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(Timeline, TimeInt)>> {
    let timelines = schema
        .index_of("timelines") // TODO
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `timelines` field`"))?;

    let timelines = timelines
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect top-level `timelines` to be a `StructArray`"))?;

    // implicit Vec<Result> to Result<Vec> collection
    let timelines: Result<Vec<_>, _> = timelines
        .fields()
        .iter()
        .zip(timelines.values())
        .map(
            |(timeline, time)| match timeline.metadata.get(TIMELINE_KEY).map(|s| s.as_str()) {
                Some(TIMELINE_TIME) => {
                    let timeline = Timeline::new(timeline.name.clone(), TimeType::Time);

                    let time = time
                        .as_any()
                        .downcast_ref::<Int64Array>()
                        .ok_or_else(|| anyhow!("expect time-like timeline to be a `Int64Array"))?;
                    ensure!(
                        time.len() == 1,
                        "expect only one timestamp per message per timeline"
                    );

                    Ok((timeline, TimeInt::from(time.values()[0])))
                }
                Some(TIMELINE_SEQUENCE) => {
                    let timeline = Timeline::new(timeline.name.clone(), TimeType::Sequence);

                    let time = time.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
                        anyhow!("expect sequence-like timeline to be a `Int64Array")
                    })?;
                    ensure!(
                        time.len() == 1,
                        "expect only one timestamp per message per timeline"
                    );

                    Ok((timeline, TimeInt::from(time.values()[0])))
                }
                Some(unknown) => {
                    bail!("unknown timeline kind: {unknown:?}")
                }
                None => {
                    bail!("missing timeline kind")
                }
            },
        )
        .collect();

    timelines
}

// TODO: is that the right data model for this? is it optimal? etc
fn extract_components<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(ComponentNameRef<'data>, &'data Box<dyn Array>)>> {
    let components = schema
        .index_of("components") // TODO
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `components` field`"))?;

    let components = components
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect component values to be `StructArray`s"))?;

    // TODO: check validity using component registry and such
    Ok(components
        .fields()
        .iter()
        .zip(components.values())
        .map(|(field, comp)| (field.name.as_str(), comp))
        .collect())
}

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
#[derive(Default, Debug)]
struct IndexTable {
    buckets: BTreeMap<TimeInt, IndexBucket>,
}

impl std::fmt::Display for IndexTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl IndexTable {
    // impl Index {
    //     pub fn insert(&mut self, time, instance_row, pos_row) {
    //         self.find_batch(time).insert(time, instance_row, pos_row)
    //     }

    //     pub fn find_batch(&mut self, time) {
    //         if let Some(bucket) = self.range(time..).next() {
    //             // if it is too big, split it in two
    //         } else {
    //             // create new bucket
    //         }
    //     }
    // }
}

/// TODO
//
// Has a max size of 128MB OR 10k rows, whatever comes first.
// The size-limit is so we can purge memory in small buckets
// The row-limit is to avoid slow re-sorting at query-time
#[derive(Debug)]
struct IndexBucket {
    /// The time range covered by this bucket.
    time_range: TimeIntRange,

    /// All indices for this bucket.
    ///
    /// Each column in this dataframe corresponds to a component.
    //
    // new columns may be added at any time
    // sorted by the first column, time (if [`Self::is_sorted`])
    //
    // TODO(cmc): some components are always present: timelines, instances
    indices: (),
}

impl std::fmt::Display for IndexBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

/// A chunked component table (i.e. a single column), bucketized by size only.
//
// The ComponentTable maps a row index to a list of values (e.g. a list of colors).
#[derive(Debug)]
struct ComponentTable {
    /// The component's name.
    name: Arc<String>,
    /// The component's datatype.
    datatype: DataType,
    /// Each bucket covers an arbitrary range of rows.
    /// How large is that range will depend on the size of the actual data, which is the actual
    /// trigger for chunking.
    buckets: BTreeMap<RowIndex, ComponentBucket>,
}

impl std::fmt::Display for ComponentTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            datatype,
            buckets,
        } = self;

        f.write_fmt(format_args!("name: {}\n", name))?;
        f.write_fmt(format_args!("datatype: {:#?}\n", datatype))?;

        f.write_str("buckets: [\n")?;
        for (_, bucket) in buckets {
            f.write_str(&indent::indent_all_by(8, "ComponentBucket {\n"))?;
            f.write_str(&indent::indent_all_by(12, bucket.to_string() + "\n"))?;
            f.write_str(&indent::indent_all_by(8, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

impl ComponentTable {
    fn new(name: String, datatype: DataType) -> Self {
        let name = Arc::new(name);
        ComponentTable {
            name: Arc::clone(&name),
            datatype: datatype.clone(),
            buckets: [(0, ComponentBucket::new(name, datatype, 0))].into(),
        }
    }

    //     pub fn push(&mut self, time_points, values) -> u64 {
    //         if self.last().len() > TOO_LARGE {
    //             self.push(ComponentTableBucket::new());
    //         }
    //         self.last().push(time_points, values)
    //     }
    pub fn insert(
        &mut self,
        timelines: &[(Timeline, TimeInt)],
        data: &Box<dyn Array>,
    ) -> anyhow::Result<RowIndex> {
        // TODO: Let's start the very dumb way: one bucket only, then we'll deal with splitting.
        self.buckets.get_mut(&0).unwrap().insert(timelines, data)
    }
}

/// TODO
//
// Has a max-size of 128MB or so.
// We bucket the component table so we can purge older parts when needed.
#[derive(Debug)]
struct ComponentBucket {
    /// The component's name.
    name: Arc<String>,

    /// The time ranges (plural!) covered by this bucket.
    ///
    /// Buckets are never sorted over time, time ranges can grow arbitrarily large.
    //
    // Used when to figure out if we can purge it.
    // Out-of-order inserts can create huge time ranges here,
    // making some buckets impossible to purge, but we accept that risk.
    //
    // TODO: this is for purging only
    time_ranges: HashMap<Timeline, TimeIntRange>, // TODO: timetype

    // TODO
    row_offset: RowIndex,

    /// All the data for this bucket. This is a single column!
    ///
    /// Each row contains the data for all instances.
    /// Instances within a row are sorted
    //
    // maps a row index to a list of values (e.g. a list of colors).
    //
    // TODO: growable array
    data: Box<dyn Array>,
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
                "    - {}: from {:?} (inclusive) to {:?} (exlusive)\n",
                timeline.name(),
                time_range.start,
                time_range.end,
            ))?;
        }

        // TODO: I'm sure there's no need to clone here
        let series = polars::prelude::Series::try_from((name.as_str(), data.clone())).unwrap();
        let df = polars::prelude::DataFrame::new(vec![series]).unwrap();
        f.write_fmt(format_args!("data: {df:?}\n"))?;

        Ok(())
    }
}

impl ComponentBucket {
    pub fn new(name: Arc<String>, datatype: DataType, row_offset: RowIndex) -> Self {
        Self {
            name,
            row_offset,
            time_ranges: Default::default(),
            data: new_empty_array(datatype),
        }
    }

    pub fn insert(
        &mut self,
        timelines: &[(Timeline, TimeInt)],
        data: &Box<dyn Array>,
    ) -> anyhow::Result<RowIndex> {
        for (timeline, time) in timelines {
            // TODO: prob should own it at this point
            let time = *time;
            let time_plus_one = time + TimeInt::from(1);
            self.time_ranges
                .entry(timeline.clone())
                .and_modify(|range| *range = range.start.min(time)..range.end.max(time_plus_one))
                .or_insert_with(|| time..time_plus_one);
        }

        // TODO: actual mutable array :)
        self.data = concatenate(&[&*self.data, &**data])?;
        // dbg!(self.data.data_type());
        // dbg!(&self.data);

        Ok(self.row_offset + self.data.len() as u64 - 1)
    }
}
