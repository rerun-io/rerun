- Start Date: 2023-03-18
- RFC PR: [rerun-io/rerun#1610](https://github.com/rerun-io/rerun/pull/1610)
- Tracking Issue: [rerun-io/rerun#1619](https://github.com/rerun-io/rerun/issues/1619)

# End-to-end batching

A design proposal for end-to-end data batches, from their creation on the client SDK and all the way until their end of life (GC).

This redesign and the major changes involved also present an opportunity to address some of the long-standing design flaws in the datastore.

- Where are we today?
- Where do we want to go and why?
- How do we get there?
- What does the future look like beyond batching?
- Try and keep note of all the good ideas that came up during discussions around batching
- Gather all the information needed for a future technical blog post about the datastore

TL;DR: A big braindump that covers a lot of the discussions and design thoughts that have been thrown around during the last few weeks, just to make sure it doesn't all get lost to time… I'm sure I've missed most of it though.

---

Status: proposal

- [Why](#why)
- [Status quo](#status-quo)
  * [Creation](#creation)
  * [Transport](#transport)
  * [Storage](#storage)
  * [Write path](#write-path)
  * [Read path](#read-path)
    + [LatestAt (random-access like)](#latestat--random-access-like-)
    + [Range (timeseries like)](#range--timeseries-like-)
  * [Garbage Collection](#garbage-collection)
  * [Save-to-disk](#save-to-disk)
- [Proposal](#proposal)
  * [Creation](#creation-1)
  * [Transport](#transport-1)
  * [Storage](#storage-1)
  * [Write path](#write-path-1)
  * [Read path](#read-path-1)
    + [LatestAt (random-access like)](#latestat--random-access-like--1)
    + [Range (timeseries like)](#range--timeseries-like--1)
  * [Garbage Collection](#garbage-collection-1)
  * [Save-to-disk](#save-to-disk-1)
- [Implementation plan](#implementation-plan)
- [Future work](#future-work)
  * [Bucket compaction making a return](#bucket-compaction-making-a-return)
  * [Dedicated storage for timeseries](#dedicated-storage-for-timeseries)
  * [Recursive clears native to the store](#recursive-clears-native-to-the-store)
  * [Native file format for writing the store to disk](#native-file-format-for-writing-the-store-to-disk)
  * [Derived components & components converter mappings](#derived-components---components-converter-mappings)
  * [Don't send full schemas of known/builtin components over the wire](#don-t-send-full-schemas-of-known-builtin-components-over-the-wire)
  * [Post-GC latest-at correctness](#post-gc-latest-at-correctness)
  * [Optimize linear backwards walk](#optimize-linear-backwards-walk)
  * [Drop-after semantics (undo/redo)](#drop-after-semantics--undo-redo-)
  * [Write our own arrow2-convert](#write-our-own-arrow2-convert)
  * [The DataStore is its own server](#the-datastore-is-its-own-server)
  * [Data might be a reference to an external storage system](#data-might-be-a-reference-to-an-external-storage-system)
- [Q&A](#q-a)
  * [What is the relationship between "component instances" and "instance keys"?](#what-is-the-relationship-between--component-instances--and--instance-keys--)
  * [Are there any "special" components?](#are-there-any--special--components-)

---

## Why

The proposed implementation aims to address several issues and provide numerous benefits, including:
- A significant reduction in the space overhead of `LogMsg`s during transport, in memory, and on disk.
- Resolution of splat issues that currently require dedicated events/rows to function.
- Resolution of the dreaded `MsgId` mismatch issues.
- Replacement of the current hackish and partially broken garbage collection mechanism with a more viable one.
- Should massively improve the speed of range queries (even more so for our scenes that span the entire time-range, e.g.: text, plot…).
- Should (likely) vastly improve the loading speed of .rrd files (even more so on the web).

Finally, these changes are expected to significantly simplify the DataStore codebase by completely eliminating component tables and their corresponding buckets.

## Status quo

This section describes the current state of things as of 2023-03-20.

The data goes through several distinct stages during its lifetime:
- Creation
- Transport
- Storage
- Write path
- Read path
- GC
- Save-to-disk

### Creation

At present, the client is limited to creating a single event at a time, corresponding to a single row of data. Each row contains N components, each of which can hold M instances for a given entity across P timelines.

To begin the process, the SDK creates a `ComponentBundle`, which can be thought of as a data cell within a dataframe. This `ComponentBundle` is essentially a list of values for a specific component type. Keep in mind we only ever work with lists, rather than individual values.
From this point forward, the individual values in these lists are referred to as "component instances" (:warning: "component instances" != "instance keys").

```rust
pub struct ComponentBundle {
    /// The name of the Component, used as column name in the table `Field`.
    name: ComponentName,

    /// The Component payload `Array`.
    value: ListArray<i32>,
}
```

These `ComponentBundle`s are then packed into a `MsgBundle`:
```rust
pub struct MsgBundle {
    /// A unique id per [`crate::LogMsg`].
    pub msg_id: MsgId,
    pub entity_path: EntityPath,
    pub time_point: TimePoint,
    pub components: Vec<ComponentBundle>,
}
```
which corresponds to _1 event_, i.e. 1 row's worth of data for 1 entity in N timelines.
This event is uniquely identified with a `MsgId`, which is a `TUID` under the hood (wall-clock UID).

The number of component instances for all columns, or components, in a given row is determined by examining the number of instances for the first entry in the `components` list. However, this approach has a significant flaw: all components must have the same number of instances.
This requirement creates a situation where splats, with a single instance, and clears, with no instances, must be sent in separate `MsgBundle`s.

As part of packing the `MsgBundle`, we convert the `MsgId` itself into a `ComponentBundle` by cloning it as many times as necessary to match the number of instances. We do this because we need the `MsgId` information later for garbage collection purposes.
However, this approach presents a challenge for clears, which have zero instances. As a result, messages that contain zero instances cannot be garbage collected as of today.

### Transport

In general, data is transmitted as an Arrow table, where each row (of which there is only ever a single one at present) represents a multi-timeline event. Each column can be either a component or a timepoint, and each cell contains a list of component instances for a given component type.

However, in practice, timepoints and components are stored separately within the chunk. This separation facilitates the identification and extraction of the time data, which receives special treatment in later stages (as discussed in the following sections).

In concrete terms, the SDK transforms the `MsgBundle` into an `ArrowMsg`, which is ready to be serialized and sent over the wire:
```rust
pub struct ArrowMsg {
    /// A unique id per [`crate::LogMsg`].
    pub msg_id: MsgId,

    /// Arrow schema
    pub schema: arrow2::Schema,

    /// Arrow chunk
    pub chunk: arrow2::Chunk<Box<dyn Array>>,
}
```

Taking a closer look at the `arrow2::Schema` of the `chunk` gives the complete story:
```rust
Schema {
    fields: [
        Field {
            name: "timelines",
            data_type: List(
                Field {
                    name: "item",
                    data_type: Struct([
                        Field { name: "timeline", data_type: Utf8, is_nullable: false, metadata: {} },
                        Field { name: "type", data_type: UInt8, is_nullable: false, metadata: {} },
                        Field { name: "time", data_type: Int64, is_nullable: false, metadata: {} },
                    ]),
                    is_nullable: true,
                    metadata: {},
                },
            ),
            is_nullable: false,
            metadata: {},
        },
        Field {
            name: "components",
            data_type: Struct([
                Field {
                    name: "rerun.text_entry",
                    data_type: List(
                        Field {
                            name: "item",
                            data_type: Struct([
                                Field { name: "body", data_type: Utf8, is_nullable: false, metadata: {} },
                                Field { name: "level", data_type: Utf8, is_nullable: true, metadata: {} },
                            ]),
                            is_nullable: true,
                            metadata: {},
                        },
                    ),
                    is_nullable: false,
                    metadata: {},
                },
                Field {
                    name: "rerun.msg_id",
                    data_type: List(
                        Field {
                            name: "item",
                            data_type: Struct([
                                Field { name: "time_ns", data_type: UInt64, is_nullable: false, metadata: {} },
                                Field { name: "inc", data_type: UInt64, is_nullable: false, metadata: {} },
                            ]),
                            is_nullable: true,
                            metadata: {},
                        },
                    ),
                    is_nullable: false,
                    metadata: {},
                },
            ]),
            is_nullable: false,
            metadata: {},
        },
    ],
    metadata: {
        "RERUN:entity_path": "logs/seg_demo_log",
    },
}
```

There are several important points to note:
- The entity path is actually passed in the schema metadata.
- This model already allows for batching, aside from the entity path just mentioned.
- We're always sending the complete schemas of our builtin components, even though they are already known to the server by definition.
- The actual instance keys can be omitted, in which case they'll be auto-generated on the server.
- Notice the extra `msg_id` column!

### Storage

The data is actually stored in two places:
- in `EntityDb`, where every raw `LogMsg` is kept around so that it can later be saved to disk,
    - Due to the lack of batching, the size of the data sitting in `EntityDb` is actually completely dwarfed by the size of the schema metadata.
- in the `DataStore`, where the data is stripped down into parts and indexed as needed for our latest-at semantics.
    - That's the origin of the `MsgId` mismatch problem.

In the store, indices are stored as tables on a per-timeline per-entity basis. To facilitate garbage collection _and_ set an upper bound on index sorting costs, these tables are further split into buckets based on both space and time thresholds:
```
IndexTable {
    timeline: frame_nr
    entity: this/that
    size: 3 buckets for a total of 256 B across 8 total rows
    buckets: [
        IndexBucket {
            index time bound: >= #0
            size: 96 B across 2 rows
                - frame_nr: from #41 to #41 (all inclusive)
            data (sorted=true):
            +----------+---------------+--------------+--------------------+
            | frame_nr | rerun.point2d | rerun.rect2d | rerun.instance_key |
            +----------+---------------+--------------+--------------------+
            | 41       | 1             |              | 2                  |
            | 41       |               | 3            | 2                  |
            +----------+---------------+--------------+--------------------+
        }
        IndexBucket {
            index time bound: >= #42
            size: 96 B across 2 rows
                - frame_nr: from #42 to #42 (all inclusive)
            data (sorted=true):
            +----------+---------------+--------------+--------------------+
            | frame_nr | rerun.point2d | rerun.rect2d | rerun.instance_key |
            +----------+---------------+--------------+--------------------+
            | 42       |               | 1            | 2                  |
            | 42       | 2             |              | 2                  |
            +----------+---------------+--------------+--------------------+
        }
        IndexBucket {
            index time bound: >= #43
            size: 64 B across 2 rows
                - frame_nr: from #43 to #44 (all inclusive)
            data (sorted=true):
            +----------+---------------+--------------+--------------------+
            | frame_nr | rerun.point2d | rerun.rect2d | rerun.instance_key |
            +----------+---------------+--------------+--------------------+
            | 43       |               | 4            | 2                  |
            | 44       | 3             |              | 2                  |
            +----------+---------------+--------------+--------------------+
        }
    ]
}
```
Note that, although the tables are bucketed, garbage collection of indices is actually entirely disabled today because the whole GC story is broken (see below).

Components are stored on a per-component basis: i.e. all timelines and all entities share the same component storage.
Like indices, they are split further into buckets (using both space and time thresholds), once again to facilitate garbage collection:
```
ComponentTable {
    name: rerun.point2d
    size: 2 buckets for a total of 96 B across 4 total rows
    buckets: [
        ComponentBucket {
            size: 64 B across 3 rows
            row range: from 0 to 0 (all inclusive)
            archived: true
            time ranges:
                - log_time: from 19:37:35.713798Z to 19:37:35.713798Z (all inclusive)
                - frame_nr: from #41 to #42 (all inclusive)
            +-------------------------------------------------------------------+
            | rerun.point2d                                                     |
            +-------------------------------------------------------------------+
            | []                                                                |
            | [{x: 2.4033058, y: 8.535466}, {x: 4.051945, y: 7.6194324}         |
            | [{x: 1.4975989, y: 6.17476}, {x: 2.4128711, y: 1.853013}          |
            +-------------------------------------------------------------------+
        }
        ComponentBucket {
            size: 32 B across 1 rows
            row range: from 3 to 3 (all inclusive)
            archived: false
            time ranges:
                - frame_nr: from #44 to #44 (all inclusive)
            +-------------------------------------------------------------------+
            | rerun.point2d                                                     |
            +-------------------------------------------------------------------+
            | [{x: 0.6296742, y: 6.7517242}, {x: 2.3393118, y: 8.770799}        |
            +-------------------------------------------------------------------+
        }
    ]
}
```
(The space thresholds don't actually work today due to the hacks we do in the GC implementation to work around `MsgId` mismatches)

Storing data in both `EntityDb` and the component tables can lead to a significant increase in memory usage if not managed carefully, effectively doubling the storage requirements. Therefore, bucket compaction is currently disabled, leaving some performance on the table.

Overall, this storage architecture maps well to our latest-at query semantics, but quite poorly to our range/timeseries semantics (see read path section below).

The index buckets in the `DataStore` hold references to specific rows in the component tables, where the actual data is stored.
At first, this may seem reasonable, but it's not the most efficient approach: Arrow data is already reference counted, so we're essentially referencing a set of references. This leads to a significant and expensive issue on the read path, particularly for range queries, as discussed below.

### Write path

The write path is fairly straightforward, with some complications arising from having to support timeless data and automatic generation of instance keys (we won't delve into those as they have no impact on batching).

First, each component (i.e., column) is inserted into the currently active component table, which generates a set of globally unique and stable row numbers.

Next, we retrieve or create the appropriate index based on the `EntityPath` and `Timeline` parameters. Using binary search, we locate the correct bucket and insert the row numbers.
That's also when bucket splitting happen, which is its own can of worms, but is completely orthogonal to batching concerns.

We also maintain an additional index that maps `MsgId`s to timepoints, which is crucial for multi-timeline views like the text view.

### Read path

#### LatestAt (random-access like)

Once again, aside from timeless considerations, latest-at queries are nothing too surprising: it's mostly a matter of grabbing the appropriate index and binsearching for the right bucket.

There are two subtleties though:
1. Finding the right data might involve linearly walking backwards (across all buckets in the worst case).
2. The result of the query is not the data itself, but rather the row numbers at which the data can be found in the component tables.

This second subtlety has important implications. To actually retrieve the data, the caller needs to perform an extra `get` request, which involves a binsearch through the component tables (and it gets costly).

#### Range (timeseries like)

While range queries have some surprisingly tricky semantics (especially around the intersection of timeless and temporal data), operationally they behave pretty much like latest-at queries: grabbing the right index, binsearching for the right bucket, and starting iteration from there.

However, the fact that we return row numbers instead of the actual data itself can have significant performance implications when it comes to range queries.
For example, if you need to iterate through 100k values, you would need to run 100k `get` requests, which would require 100k binsearches in the component tables. This can be extremely costly and is a major reason why our ranged query scenes quickly become unusable as the dataset grows.

### Garbage collection

The current garbage collection mechanism was put together as a quick fix for the `MsgId`-mismatch issue, and it is largely unreliable.

The algorithm works as follows: it finds the oldest component bucket based on the insertion order from the datastore, which doesn't make much semantic sense, and drops it. Then, it drops all component buckets that roughly cover the same time range. Finally, it returns all the `MsgId`s to the Viewer so that it can in turn clear its own data structures.
This process is repeated in a loop until a sufficient amount of data has been dropped.

Beyond these hacks, the logic in and of itself is fundamentally broken right now. Consider the following log calls:
```python
log_color("some/entity", frame_nr=0, [{255, 0, 0, 255}])
log_point("some/entity", frame_nr=1, [{1.0, 1.0}])
log_point("some/entity", frame_nr=2, [{2.0, 2.0}])
log_point("some/entity", frame_nr=3, [{3.0, 3.0}])
log_point("some/entity", frame_nr=4, [{4.0, 4.0}])
log_point("some/entity", frame_nr=5, [{5.0, 5.0}])
```

Querying for `LatestAt("some/entity", ("frame_nr", 5))` will unsurprisingly yield a red point at `(5.0, 5.0)`.

Now, consider what happens after running a GC that drops 50% of the data, leaving us with:
```python
log_point("some/entity", frame_nr=3, [{3.0, 3.0}])
log_point("some/entity", frame_nr=4, [{4.0, 4.0}])
log_point("some/entity", frame_nr=5, [{5.0, 5.0}])
```

Querying for `LatestAt("some/entity", ("frame_nr", 5))` will now yield a point at `(5.0, 5.0)` with whatever is currently defined as the default color, rather than red. This is just plain wrong.

This happens because the GC blindly drops data rather than doing the correct thing: compacting what gets dropped into a latest-at kind of state and keeping that around for future queries.

### Save-to-disk

The current store cannot be dumped to disk, we rely on `EntityDb` to store all incoming `LogMsg`s and dump them to disk as-is if the user decides to save the recording.

---

## Proposal

The proposed design involves significant changes at every stage of the data lifecycle.

### Creation

The main difference is of course that the client can now accumulate events (i.e., rows) in a local table before sending them to the server.
In practice this process of accumulation is handled behind the scenes by the SDK, and driven by both time and space thresholds ("accumulate at most 10MiB of raw data for no more than 50ms").

To reflect the fact that we're passing tables of data around, I suggest we update the terminology.
The current terms `ComponentBundle` and `MsgBundle` are vague, so let's use more descriptive terms instead:
* `DataCell`: a uniform list of values for a given component type from a single log call.
* `DataRow`: an event, a list of cells associated with an event ID, entity path, timepoint, and number of instances. Corresponds to a single SDK log call.
* `DataTable`: a batch; a list of rows associated with a batch ID.

Juggling between native and Arrow data interchangeably can be a cumbersome task in our current implementation. While we have some helper functions to facilitate this, the process is not as smooth as it could be.
This is partly due to limitations in `arrow2-convert`, but also because some of our APIs are simply not optimized for this use case (yet).

So, considering all the reasons above, here are all the new types involved.

`DataCell`, which roughly fills the role of our current `ComponentBundle`:
```rust
/// A cell's worth of data, i.e. a uniform list of values for a given component type: `[C]`.
pub struct DataCell {
    /// Name of the component type used in this cell.
    //
    // TODO(cmc): We should consider storing this information within the values array itself, rather than
    // outside of it. Arrow has the concept of extensions specifically for storing type metadata, but
    // we have had some issues with it in the past. This is an opportunity to revisit and improve upon
    // that implementation.
    name: ComponentName,

    /// A uniformly typed list of values for the given component type.
    ///
    /// Includes the data, its schema and probably soon the component metadata (e.g. the `ComponentName`).
    values: Box<dyn arrow2::Array>,
}

impl DataCell {
    /// Builds a new `DataCell` out of a uniform list of native component values.
    pub fn from_native<C: Component>(values: Vec<C>) -> Self { /* … */ }

    /// Builds a new `DataCell` from an arrow array.
    //
    // TODO(cmc): We shouldn't have to specify the component type separately, this should be part of the
    // metadata by using an extension.
    pub fn from_arrow(name: ComponentName, values: Box<dyn arrow2::Array>) -> Self  { /* … */ }

    /// Builds an empty `DataCell` from an arrow datatype.
    //
    // TODO(cmc): We shouldn't have to specify the component type separately, this should be part of the
    // metadata by using an extension.
    pub fn from_datatype(name: ComponentName, datatype: DataType) -> Self  { /* … */ }

    /// Builds an empty `DataCell` from a component type.
    //
    // TODO(cmc): do keep in mind there's a future not too far away where components become a
    // `(component, type)` tuple kinda thing.
    pub fn from_component<C: Component>() -> Self  { /* … */ }

    /// Returns the contents of the cell as an arrow array.
    pub fn as_arrow(&self) -> Box<dyn arrow2::Array> { /* … */ }

    /// Returns the contents of the cell as vector of native components.
    //
    // TODO(cmc): We could potentially keep the original native component values if the cell was created
    // using `from_native`.
    pub fn as_components<C: Component>(&self) -> Vec<C> { /* … */ }
}

// TODO(cmc): Some convenient `From` implementations etc
```
(The "arrow extension" thing that is mentioned a lot in the comments above is [this](https://docs.rs/arrow2/latest/arrow2/datatypes/enum.DataType.html#variant.Extension).)

`DataRow`, which fills the shoes of today's `MsgBundle`:
```rust
/// A row's worth of data, i.e. an event: a list of [`DataCell`]s associated with an auto-generated
/// [`EventId`], a user-specified [`TimePoint`] and [`EntityPath`], and an expected number of
/// instances.
pub struct DataRow {
    /// Auto-generated [`TUID`], uniquely identifying this event and keeping track of the client's
    /// wall-clock.
    event_id: EventId,

    /// User-specified [`TimePoint`] for this event.
    timepoint: TimePoint,

    /// User-specified [`EntityPath`] for this event.
    entity_path: EntityPath,

    /// The expected number of values (== component instances) in each cell.
    ///
    /// Each cell must have either:
    /// - 0 instance (clear),
    /// - 1 instance (splat),
    /// - `num_instances` instances (standard).
    num_instances: u32,

    /// The actual cells (== columns).
    cells: Vec<DataCell>,
}

impl DataRow {
    /// Builds a new `DataRow` out of a list of [`DataCell`]s.
    pub fn from_cells(
        timepoint: TimePoint,
        entity_path: EntityPath,
        num_instances: u32,
        cells: Vec<DataCell>,
    ) -> Self { /* … */ }

    /// Append a cell to an existing row.
    ///
    /// Returns an error if the cell is not compatible with the row, e.g.:
    /// - Trying to append a cell which contains neither `0`, `1` or `num_instances`.
    /// - Trying to append the same component type more than once.
    /// - Etc.
    pub fn append_cell(&mut self, cell: DataCell) -> Result<()> { /* … */ }
}

// TODO(cmc): Some convenient `From` implementations etc
```

And finally `DataTable`, which is where the batching happens:
```rust
/// An entire table's worth of data, i.e. a batch: a list of [`DataRow`]s associated with an auto-generated
/// [`BatchId`].
struct DataTable {
    /// Auto-generated [`TUID`], uniquely identifying this batch of data and keeping track of the
    /// client's wall-clock.
    batch_id: BatchId,

    /// The entire column of [`EventId`]s.
    event_id: Vec<EventId>,

    /// The entire column of [`TimePoint`]s.
    timepoint: Vec<TimePoint>,

    /// The entire column of [`EntityPath`]s.
    entity_path: Vec<EntityPath>,

    /// The entire column of `num_instances`.
    num_instances: Vec<u32>,

    /// All the rows for all the component columns.
    ///
    /// The cells are optional since not all rows will have data for every single component (i.e. the table is sparse).
    rows: HashMap<ComponentName, Vec<Option<DataCell>>>,
}

impl DataTable {
    /// Builds a new `DataTable` out of a list of [`DataRow`]s.
    pub fn from_rows(rows: Vec<DataRow>) -> Self { /* … */ }

    /// Append a row to an existing table.
    ///
    /// Returns an error if the row is not compatible with the table.
    pub fn append_row(&mut self, row: DataRow) -> Result<()> { /* … */ }
}

// TODO(cmc): Some convenient `From` implementations etc
```

These datastructures should get rid of all the issues that plague clears, splats and everything that ensue from `MsgId` mismatch issues.

The SDK accumulates cells into rows into tables until either the space or time thresholds are reached, at which point the batch is ready for transport.

Note that `DataCell`, `DataRow`, `DataTable` are all temporary constructs to help with the creation of data batches, they are not what gets sent over the wire (although `DataCell` pre-serializes its data as it is much more convenient to erase component data before passing it around).

Only when a `DataTable` gets transformed into a `ArrowMsg` does serialization actually happen.
`ArrowMsg` is what gets sent over the wire.

### Transport

`ArrowMsg` stays roughly the same in spirit: it's the fully serialized Arrow representation of a `DataTable`:
```rust
pub struct ArrowMsg {
    /// Auto-generated [`TUID`], uniquely identifying this batch of data and keeping track of the
    /// client's wall-clock.
    pub batch_id: BatchId,

    /// The schema for the entire table.
    pub schema: arrow2::Schema,

    /// The data for the entire table.
    pub chunk: arrow2::Chunk<Box<dyn Array>>,
}
```

The new schema is expected to look like this (-ish):
```rust
Schema {
    fields: [
        Field {
            name: "event_id",
            data_type: List(
                Field {
                    name: "item",
                    data_type: Struct([
                        Field { name: "time_ns", data_type: UInt64, is_nullable: false, metadata: {} },
                        Field { name: "inc", data_type: UInt64, is_nullable: false, metadata: {} },
                    ]),
                    is_nullable: true,
                    metadata: { "rerun.kind": "event_id" },
                },
            ),
            is_nullable: false,
            metadata: {},
        },
        // TODO(cmc): not the right type but you get the idea
        Field {
            name: "num_instances",
            data_type: List(
                Field {
                    name: "item",
                    data_type: UInt32,
                    is_nullable: false,
                    metadata: { "rerun.kind": "num_instances" },
                },
            ),
            is_nullable: false,
            metadata: {},
        },
        // TODO(cmc): not the right type but you get the idea
        Field {
            name: "entity_path",
            data_type: List(
                Field {
                    name: "item",
                    data_type: Utf8,
                    is_nullable: false,
                    metadata: { "rerun.kind": "entity_path" }
                },
            ),
            is_nullable: false,
            metadata: {},
        },
        Field {
            name: "timepoint",
            data_type: List(
                Field {
                    name: "item",
                    data_type: Struct([
                        Field { name: "timeline", data_type: Utf8, is_nullable: false, metadata: {} },
                        Field { name: "type", data_type: UInt8, is_nullable: false, metadata: {} },
                        Field { name: "time", data_type: Int64, is_nullable: false, metadata: {} },
                    ]),
                    is_nullable: true,
                    metadata: {},
                },
            ),
            is_nullable: false,
            metadata: {},
        },
        Field {
            name: "components",
            data_type: Struct([
                Field {
                    name: "text_entry",
                    data_type: List(
                        Field {
                            name: "item",
                            data_type: Struct([
                                Field { name: "body", data_type: Utf8, is_nullable: false, metadata: {} },
                                Field { name: "level", data_type: Utf8, is_nullable: true, metadata: {} },
                            ]),
                            is_nullable: true,
                            metadata: {
                                "rerun.kind": "component".
                                "rerun.component": "rerun.text_entry",
                            },
                        },
                    ),
                    is_nullable: false,
                    metadata: {},
                },
            ]),
            is_nullable: false,
            metadata: {},
        },
    ],
    metadata: {
        "rerun.batch_id": "<BatchId>",
    },
}
```

The one major difference here is that `event_id`, `entity_path` and `num_instances` join `timepoints` in having dedicated, top-level columns.
This is important as, like timepoints, these will have to be handled separately (and deserialized into native types!) in order to drive some of the logic in the store.

Lastly, we inject the `BatchId` as metadata.
`BatchId` isn't used for any logic yet, but comes in very handy for debug purposes.

At this point we might want to sort the batch by `(event_id, entity_path)`, which will greatly improve data locality once it sits in the store (see storage section below).

That's also an opportunity to pre-compact the data: if two rows share the same timepoints with different components, we could potentially merge them together… that's a bit more controversial though as it means either dropping some `EventId`s, or supporting multiple `EventId`s for a single event.

One last thing that needs to be taken care of before actually sending the data is compression / dictionary-encoding of some kind.
We already have `zstd` in place for that.

### Storage

One of the major change storage-wise is the complete removal of component tables: index tables now reference the Arrow data directly.
With the new design, the Arrow buffers now store multiple rows of data. To reference a specific row, each index row must point to _a unit-length slice_ in a shared batch of Arrow data.

That is the reason why sorting the batch on the client's end improves performance: it improves data locality in the store by making the shared batches follow the layout of the final buckets more closely.

Assuming the following syntax for Arrow slices: `ArrowSlice(<buffer_adrr, offset>)`, indices should now look roughly like the following, sorted by the timeline (`frame_nr`):
```
IndexTable {
    timeline: frame_nr
    entity: this/that
    size: 3 buckets for a total of 256 B across 8 total rows
    buckets: [
        IndexBucket {
            index time bound: >= #0
            size: 96 B across 2 rows
                - frame_nr: from #41 to #41 (all inclusive)
            data (sorted=true):
            +----------+----------+---------------+-------------------+-------------------+------------------------+
            | event_id | frame_nr | num_instances | rerun.point2d     | rerun.rect2d      | rerun.instance_key     |
            +----------+----------+---------------+-------------------+-------------------+------------------------+
            | 1        | 41       | 2             | ArrowSlice(1,33)  |                   | ArrowSlice(0, 0)       |
            | 2        | 41       | 2             |                   | ArrowSlice(1,25)  | ArrowSlice(0, 0)       |
            +----------+----------+---------------+-------------------+-------------------+------------------------+
        }
        IndexBucket {
            index time bound: >= #42
            size: 96 B across 2 rows
                - frame_nr: from #42 to #42 (all inclusive)
            data (sorted=true):
            +----------+----------+---------------+-------------------+-------------------+------------------------+
            | event_id | frame_nr | num_instances | rerun.point2d     | rerun.rect2d      | rerun.instance_key     |
            +----------+----------+---------------+-------------------+-------------------+------------------------+
            | 3        | 42       | 2             |                   | ArrowSlice(2,25)  | ArrowSlice(0, 0)       |
            | 4        | 42       | 2             | ArrowSlice(2,33)  |                   | ArrowSlice(0, 0)       |
            +----------+----------+---------------+-------------------+-------------------+------------------------+
        }
        IndexBucket {
            index time bound: >= #43
            size: 64 B across 2 rows
                - frame_nr: from #43 to #44 (all inclusive)
            data (sorted=true):
            +----------+----------+---------------+-------------------+-------------------+------------------------+
            | event_id | frame_nr | num_instances | rerun.point2d     | rerun.rect2d      | rerun.instance_key     |
            +----------+----------+---------------+-------------------+-------------------+------------------------+
            | 6        | 43       | 2             |                   | ArrowSlice(3,25)  | ArrowSlice(0, 0)       |
            | 5        | 44       | 2             | ArrowSlice(3,33)  |                   | ArrowSlice(0, 0)       |
            +----------+----------+---------------+-------------------+-------------------+------------------------+
        }
    ]
}
```

Worth noticing:
- `event_id` and `num_instances` are deserialized and stored natively, as they play a crucial role in many storage and query features.
- In this example, `rerun.instance_key` consistently references the same slice of Arrow data. This is because they are auto-generated in this case.

In addition to storing the indices themselves, we also require a bunch of auxiliary datastructures.

First, we need to keep track of all `EventId`s currently present in the store, in `event_id` order (remember, these are time-based (clients' wall-clocks)!).
This will replace the existing `chronological_msg_ids` in `EntityDb` (which is currently in insertion-order-as-seen-from-the-viewer, which isn't too great).
We need this because some operations like GC and save-to-disk require to pick an arbitrary ordering to get going, and `event_id` is our best bet for now.

Second, we need to map `EventId`s to index rows for GC purposes: `HashMap<EventId, HashSet<IndexRowId>>`.

Finally, we need to map `EventId`s to `TimePoint`s: `HashMap<EventId, TimePoint>`.
This is something we already have today and that is needed for e.g. text log views.

Overall this is a much simpler design, and while it still isn't optimal for timeseries-like queries (i.e. range), it should already quite the bump in performance for those.

### Write path

For each row in the batch, we create a bunch of unit-length Arrow slices that point to the right place in the shared buffer, and then it's all pretty much the same as before.

The major difference is we now directly store Arrow buffers (which are really Arrow slices under the hood) rather than indices into component tables.
Everything else is the same: get (or create) the appropriate index (`EntityPath` + `Timeline`), find the right bucket using a binsearch, and insert those Arrow slices

We also actually deserialize the `event_id` and `num_instances` columns into native types as we're going to need those for the store to function:
- `event_id`s are needed to maintain our auxiliary datastructures (GC, save-to-file)
- `num_instances` are needed for `re_query` to be able to do its job

### Read path

The major difference is we now directly return Arrow buffers (which are really Arrow slices under the hood), which means `get` queries are gone… which means latest-at queries should get a bit faster and range queries should get much faster.

Everything else is the same: grab the right index (`EntityPath` + `Timeline`), binsearch for the right bucket, walk backwards if you need to, and you're done.

An important difference is we're now returning the expected number of instances as part of the result, which means `re_query` doesn't have to guess anymore and can actually apply clears and splats appropriately.

#### LatestAt (random-access like)

Nothing specific to add to the above.

#### Range (timeseries like)

Nothing specific to add to the above.

### Garbage collection

The garbage collector is the system undergoing the most changes.

We want to garbage collect in `event_id` order (reminder: `EventId` is a `TUID`, i.e. its order is based on the clients's wall-clocks).

- We iterate over all `EventId`s in their natural order
- For every one of them:
    - Find all index rows that match this `event_id` and replace the Arrow slice with a tombstone, this has 2 effects:
        1. This decrements the internal refcount of the overall Arrow buffer, which might deallocate it if this happens to be the last one standing
        2. This lets the read path knows it should ignore this row (and maybe not look any further?)
    - Check if the bucket now only contains tombstones, and drop it entirely if that's the case
    - Remove the freshly dropped `event_id` from all our auxiliary datastructures
    - Measure how much memory has been reclaimed (if any!) and decide whether we should continue with the next `event_id` on the list
- Return all the time ranges that have been dropped to the main app so that it can update the timeline widget appropriately

### Save-to-disk

The store is now in charge of saving to disk, i.e. we do _not_ store raw `LogMsg`s in the main app anymore.

The dumping process is very similar to how the new GC works:
- We iterate over all `EventId`s in their natural order
- For every one of them:
    - Find all index rows that match this `event_id` and:
        1. Merge their timepoints (the rest of the data must be identical!)
        2. Reconstruct a `LogMsg` using the data from the row
- Return all these `LogMsg`s to the main save-to-disk function

Doing the above results in something functional and technically correct.. but there's a catch: we're losing all the original batching, which means re-loading this .rrd file will have poorer performance than when exploring the original recording.
Not only we do not want to lose the original batching, ideally we would want to improve on it, i.e. batch even more aggressively when we're writing to disk!

We want an extra step in there: accumulate rows of data until we reach a given size, and then craft a single `LogMsg` out of that.
This will make the resulting .rrd file both faster to load (there's some fixed overhead for each `LogMsg`, especially on web…) and faster to explore (improved cache locality in the store).

## Implementation plan

A _lot_ of things are gonna change, so we really want A) to avoid crazy large PRs that are a pain to review and B) to be able detect regressions (both correctness and performance) early on so they don't turn into long & painful investigations.

Nothing set in stone obviously, but the following steps seem like a good start (roughly 1 step == 1 PR).
This entire section can pretty much be used verbatim as a tracking issue.

1. Implement all the needed tests & benchmarks
We need to be able to check for regressions at every step, so make sure we have all the tests and benchmarks we need for that.
We should already be 95% of the way there at this point.

1. Move `DataStore` sanity checks and formatting tools to separate files
`store.rs` is supposed to be the place where one can get an overview of all the datastructures involved in the store, except it has slowly become a mess over time and is now pretty much unreadable.

1. Replace `MsgBundle` & `ComponentBundle` with the new types (`DataCell`, `DataRow`, `DataTable`, `EventId`, `BatchId`…)
No actual batching features nor any kind of behavior changes of any sort: just define the new types and use them everywhere.

1. Pass entity path as a column rather than as metadata
Replace the current entity_path that is passed in the metadata map with an actual column instead. This will also requires us to make `EntityPath` a proper Arrow datatype (..datatype, not component!!).

1. Implement explicit number of instances
Introduce a new column for `num_instances`, integrate it in the store index and expose it in the store APIs.

1. Fix splats all around (rs sdk, py sdk, re_query…)
Update the SDKs and `re_query` to properly make use of the new explicit `num_instances`.

1. Get rid of component buckets altogether
Update the store implementation to remove component tables, remove the `get` APIs, introduce slicing on the write path, etc. Still no batching in sight!

1. Implement the coalescing/accumulation logic in the SDK
Add the required logic/thread/timers/whatever-else in the SDKs to accumulate data and just send it all as many `LogMsg`s (i.e. no batching yet).

1. Implement full-on batching
End-to-end: transport, storage, the whole shebang.

1. Sort the batch before sending (`(event_id, entity_path)`)
Keep that in its own PR to keep track of the benchmarks.

1. Implement new GC
The complete implementation; should close all existing GC issues.

1. Dump directly from the store into an rrd file
No rebatching yet, just dump every event in its own `LogMsg`.

1. Remove `LogMsg`s from `EntityDb`
We shouldn't need to keep track of events outside the store past this point: clean it all up.
Reminder: the timeline widget keeps track of timepoints directly, not events.

1. Rebatch aggressively while dumping to disk

1. Use Arrow extension types to carry around component names

1. Drop `log_time`
We currently store the logging time twice: once in the `MsgId` (soon `EventId`) and once injected by the SDK (and they don't even match!).
We could just not inject the `log_time`, and instead derive a `log_time` column on the server using the timestamp in the `EventId`; especially since we probably want an auto-derived `ingestion_time` anyway.
The timestamp in `EventId` is not going away: it is what defines our global ordering!

- Turn all of the above into a tracking issue
- Get to work :>

<!-- - Merge rows / pre-compact in the batch -->

## Future work

Future work that we're expecting to happen in the mid-term and that we should keep in mind while making changes to the datastore design, so we don't end up completely stuck later on.

- Might or might not be related to batching
- No particular order

### Bucket compaction making a return

Basically aggressive rebatching on the loaded data.

### Dedicated storage for timeseries

While our store design nicely matches latest-at semantics, it's pretty horrible when it comes to range/timeseries-like semantics.
It gets even worse for timeseries of simple scalars.

At some point we're gonna want to have a fully dedicated storage & query path for scalar timeseries.

### Recursive clears native to the store

Recursive clears are currently handled in `EntityDb`, which is an issue for (at least) two reasons:
- Once we start saving the store in a native format, rather than a collection of `LogMsg`, we'll lose the recursive clears when dumping then reloading the recording.
- The recursive clears aren't even arrowified yet.

### Native file format for writing the store to disk

We currently store recordings as a stream of `LogMsg`s, which is nice for some purposes and awful for others.

While we still want to have the ability to dump the store as a stream of `LogMsg`, in the future we will need a native format that allows streaming data in and out of the disk as needed.

### Derived components & components converter mappings

We'd like to have components be generic over their datatype at some point, and be able to register conversion routines to map from `(component A, type B)` to `(component C, type D)`.

### Don't send full schemas of known/builtin components over the wire

Builtin components have fixed schemas, sending that information everytime is wasteful.

### Post-GC latest-at correctness

The current garbage collector is factually wrong; consider the following state:
```
+----------+----------+---------------+-------------------+-------------------+------------------------+
| event_id | frame_nr | num_instances | rerun.point2d     | rerun.rect2d      | rerun.instance_key     |
+----------+----------+---------------+-------------------+-------------------+------------------------+
| 1        | 40       | 2             | ArrowSlice(1,33)  |                   | ArrowSlice(0, 0)       |
| 2        | 41       | 2             |                   | ArrowSlice(1,25)  | ArrowSlice(0, 0)       |
+----------+----------+---------------+-------------------+-------------------+------------------------+
```

A `LatestAt(("frame_nr", 42))` query would yield `[rerun.point2d=ArrowSlice(1,33), rerun.rect2d=ArrowSlice(1,25)]`.

Now let's say we run a GC with `AtLeast(0.30)` ("collect at least 30%"), we end up with the following state:
```
+----------+----------+---------------+-------------------+-------------------+------------------------+
| event_id | frame_nr | num_instances | rerun.point2d     | rerun.rect2d      | rerun.instance_key     |
+----------+----------+---------------+-------------------+-------------------+------------------------+
| 2        | 41       | 2             |                   | ArrowSlice(1,25)  | ArrowSlice(0, 0)       |
+----------+----------+---------------+-------------------+-------------------+------------------------+
```

A `LatestAt(("frame_nr", 42))` query would now yield `[rerun.rect2d=ArrowSlice(1,25)]`, i.e. `rerun.point2d` would now fallback to its default value, rather than whatever value was in `ArrowSlice(1,33)`.

When garbage collecting, we _have to_ keep track of the compacted latest-at state that would have been there otherwise.

### Optimize linear backwards walk

Although this has been fixed in the trivial case (the component is not present at all), this can still be an issue in others.
The classic solution is some kind of bitmap index.

### Drop-after semantics (undo/redo)

If we decide to retain the UI's undo/redo state in the store, we will require a method to discard all data from a certain point in time and beyond.

I.e. a GC request of the form `DropAfter(("frame_nr", 41))`, rather than the other way around that our GC already supports.

### Write our own arrow2-convert

`arrow2-convert` has a bunch of shortcomings that are likely going to push us to write our own struct-to-arrow mapper in the future, and as such we shouldn't restrain ourselves based on `arrow2-convert`'s design.

### The DataStore is its own server

The datastore won't run as part of the Viewer forever.

### Data might be a reference to an external storage system

At some point we're going need the ability for components to refer to data that reside out of the store (e.g. a component `VideoFrame` which is merely a URI pointing to a video file "somewhere").

### Non-integer instance keys

Instance keys currently only support `u32` as their datatype. Maybe at some point we'll want to support others..?

### Streamed serialization

Don't waste compute & memory on creating large `arrow::Chunk`s from cells, instead serialize them independently in a streaming fashion.

## Q&A

### What is the relationship between "component instances" and "instance keys"?

> It seems like we do treat instance keys like any other component, which means each individual instance key is actually a component instance, no?

The terminology is very subtle.

- `InstanceKey` is indeed a component, and so it is always passed as a list, which we colloquially refer to as "the instance keys".
- a "component instance", or just "instance", is the name we give to any single value in a component cell:
```
[C, C, …]
 ^  ^
 |  |
 |  |
 |  instance #2
 |
 instance #1
```

So, a cell of `InstanceKey`s is indeed made up of component instances, but "instance keys" and "component instances" are two different things.

> each individual instance key is actually a component instance, no?

Yes, each individual instance key is a component instance, but not every component instance is necessarily an `InstanceKey`.

### Are there any "special" components?

> Does `DataRow::cells` include the "instance key" component then? Are there any other special components?

The following list is all the types that are treated separately because they need to be deserialized and stored natively in the `DataStore` since they drive its behavior:
```rust
event_id: Vec<EventId>,
timepoint: Vec<TimePoint>,
entity_path: Vec<EntityPath>,
num_instances: Vec<u32>,
```
None of these are components however, they are merely Arrow datatypes.

Everything else is just a component, and as such is passed as a `DataCell`.

Components are completely opaque blobs from the store's PoV; they cannot dictate its behavior since they aren't even deserialized.
This includes `InstanceKey`s, which are just returned to `re_query` as-is.

The one special thing about instance keys is that they are auto-generated server-side if they are missing; but even then, once they are generated they are still treated as opaque blobs from the store's PoV.
