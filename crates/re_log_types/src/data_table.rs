use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Itertools as _;
use nohash_hasher::IntSet;
use smallvec::SmallVec;

use crate::{
    ArrowMsg, ComponentName, DataCell, DataCellError, DataRow, DataRowError, EntityPath, RowId,
    SizeBytes, TimePoint, Timeline,
};

// ---

#[derive(thiserror::Error, Debug)]
pub enum DataTableError {
    #[error("Trying to deserialize data that is missing a column present in the schema: {0:?}")]
    MissingColumn(String),

    #[error(
        "Trying to deserialize time column data with invalid datatype: {name:?} ({datatype:#?})"
    )]
    NotATimeColumn { name: String, datatype: DataType },

    #[error("Trying to deserialize column data that doesn't contain any ListArrays: {0:?}")]
    NotAColumn(String),

    #[error("Error with one or more the underlying data rows: {0}")]
    DataRow(#[from] DataRowError),

    #[error("Error with one or more the underlying data cells: {0}")]
    DataCell(#[from] DataCellError),

    #[error("Could not serialize/deserialize component instances to/from Arrow: {0}")]
    Arrow(#[from] arrow2::error::Error),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type DataTableResult<T> = ::std::result::Result<T, DataTableError>;

// ---

pub type RowIdVec = SmallVec<[RowId; 4]>;

pub type TimeOptVec = SmallVec<[Option<i64>; 4]>;

pub type TimePointVec = SmallVec<[TimePoint; 4]>;

pub type ErasedTimeVec = SmallVec<[i64; 4]>;

pub type EntityPathVec = SmallVec<[EntityPath; 4]>;

pub type NumInstancesVec = SmallVec<[u32; 4]>;

pub type DataCellOptVec = SmallVec<[Option<DataCell>; 4]>;

/// A column's worth of [`DataCell`]s: a sparse collection of [`DataCell`]s that share the same
/// underlying type and likely point to shared, contiguous memory.
///
/// Each cell in the column corresponds to a different row of the same column.
#[derive(Debug, Clone, PartialEq)]
pub struct DataCellColumn(pub DataCellOptVec);

impl std::ops::Deref for DataCellColumn {
    type Target = [Option<DataCell>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// TODO(cmc): Those Deref don't actually do their job most of the time for some reason...

impl std::ops::DerefMut for DataCellColumn {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Index<usize> for DataCellColumn {
    type Output = Option<DataCell>;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for DataCellColumn {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl DataCellColumn {
    #[inline]
    pub fn empty(num_rows: usize) -> Self {
        Self(smallvec::smallvec![None; num_rows])
    }

    /// Compute and cache the size of each individual underlying [`DataCell`].
    /// This does nothing for cells whose size has already been computed and cached before.
    ///
    /// Beware: this is _very_ costly!
    #[inline]
    pub fn compute_all_size_bytes(&mut self) {
        for cell in &mut self.0 {
            cell.as_mut().map(|cell| cell.compute_size_bytes());
        }
    }
}

impl SizeBytes for DataCellColumn {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

// ---

/// A unique ID for a [`DataTable`].
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    arrow2_convert::ArrowField,
    arrow2_convert::ArrowSerialize,
    arrow2_convert::ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct TableId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TableId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);

    #[inline]
    pub fn random() -> Self {
        Self(re_tuid::Tuid::random())
    }
}

impl SizeBytes for TableId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl std::ops::Deref for TableId {
    type Target = re_tuid::Tuid;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for TableId {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A sparse table's worth of data, i.e. a batch of events: a collection of [`DataRow`]s.
/// This is the top-level layer in our data model.
///
/// Behind the scenes, a `DataTable` is organized in columns, where columns are represented by
/// sparse lists of [`DataCell`]s.
/// Cells within a single list are likely to reference shared, contiguous heap memory.
///
/// Cloning a `DataTable` can be _very_ costly depending on the contents.
///
/// ## Field visibility
///
/// To facilitate destructuring (`let DataTable { .. } = row`), all the fields in `DataTable` are
/// public.
///
/// Modifying any of these fields from outside this crate is considered undefined behavior.
/// Use the appropriate getters and setters instead.
///
/// ## Layout
///
/// A table is a collection of sparse rows, which are themselves collections of cells, where each
/// cell must either be empty (a clear), unit-lengthed (a splat) or `num_instances` long
/// (standard):
/// ```text
/// [
///   [[C1, C1, C1], [], [C3], [C4, C4, C4], ...],
///   [None, [C2, C2], [], [C4], ...],
///   [None, [C2, C2], [], None, ...],
///   ...
/// ]
/// ```
///
/// Consider this example:
/// ```ignore
/// let row0 = {
///     let num_instances = 2;
///     let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
///     let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
///     let labels: &[Label] = &[];
///     DataRow::from_cells3(RowId::random(), "a", timepoint(1, 1), num_instances, (points, colors, labels))
/// };
/// let row1 = {
///     let num_instances = 0;
///     let colors: &[ColorRGBA] = &[];
///     DataRow::from_cells1(RowId::random(), "b", timepoint(1, 2), num_instances, colors)
/// };
/// let row2 = {
///     let num_instances = 1;
///     let colors: &[_] = &[ColorRGBA::from_rgb(255, 255, 255)];
///     let labels: &[_] = &[Label("hey".into())];
///     DataRow::from_cells2(RowId::random(), "c", timepoint(2, 1), num_instances, (colors, labels))
/// };
/// let table = DataTable::from_rows(table_id, [row0, row1, row2]);
/// ```
///
/// A table has no arrow representation nor datatype of its own, as it is merely a collection of
/// independent rows.
///
/// The table above translates to the following, where each column is contiguous in memory:
/// ```text
/// ┌──────────┬───────────────────────────────┬──────────────────────────────────┬───────────────────┬─────────────────────┬─────────────┬──────────────────────────────────┬─────────────────┐
/// │ frame_nr ┆ log_time                      ┆ rerun.row_id                     ┆ rerun.entity_path ┆ rerun.num_instances ┆ rerun.label ┆ rerun.point2d                    ┆ rerun.colorrgba │
/// ╞══════════╪═══════════════════════════════╪══════════════════════════════════╪═══════════════════╪═════════════════════╪═════════════╪══════════════════════════════════╪═════════════════╡
/// │ 1        ┆ 2023-04-05 09:36:47.188796402 ┆ 1753004ACBF5D6E651F2983C3DAF260C ┆ a                 ┆ 2                   ┆ []          ┆ [{x: 10, y: 10}, {x: 20, y: 20}] ┆ [2155905279]    │
/// ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 1        ┆ 2023-04-05 09:36:47.188852222 ┆ 1753004ACBF5D6E651F2983C3DAF260C ┆ b                 ┆ 0                   ┆ -           ┆ -                                ┆ []              │
/// ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 2        ┆ 2023-04-05 09:36:47.188855872 ┆ 1753004ACBF5D6E651F2983C3DAF260C ┆ c                 ┆ 1                   ┆ [hey]       ┆ -                                ┆ [4294967295]    │
/// └──────────┴───────────────────────────────┴──────────────────────────────────┴───────────────────┴─────────────────────┴─────────────┴──────────────────────────────────┴─────────────────┘
/// ```
///
/// ## Example
///
/// ```rust
/// # use re_log_types::{
/// #     component_types::{ColorRGBA, Label, Point2D},
/// #     DataRow, DataTable, RowId, TableId, Timeline, TimePoint,
/// # };
/// #
/// # let table_id = TableId::random();
/// #
/// # let timepoint = |frame_nr: i64, clock: i64| {
/// #     TimePoint::from([
/// #         (Timeline::new_sequence("frame_nr"), frame_nr.into()),
/// #         (Timeline::new_sequence("clock"), clock.into()),
/// #     ])
/// # };
/// #
/// let row0 = {
///     let num_instances = 2;
///     let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
///     let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
///     let labels: &[Label] = &[];
///
///     DataRow::from_cells3(
///         RowId::random(),
///         "a",
///         timepoint(1, 1),
///         num_instances,
///         (points, colors, labels),
///     )
/// };
///
/// let row1 = {
///     let num_instances = 0;
///     let colors: &[ColorRGBA] = &[];
///
///     DataRow::from_cells1(RowId::random(), "b", timepoint(1, 2), num_instances, colors)
/// };
///
/// let row2 = {
///     let num_instances = 1;
///     let colors: &[_] = &[ColorRGBA::from_rgb(255, 255, 255)];
///     let labels: &[_] = &[Label("hey".into())];
///
///     DataRow::from_cells2(
///         RowId::random(),
///         "c",
///         timepoint(2, 1),
///         num_instances,
///         (colors, labels),
///     )
/// };
///
/// let table_in = DataTable::from_rows(table_id, [row0, row1, row2]);
/// eprintln!("Table in:\n{table_in}");
///
/// let (schema, columns) = table_in.serialize().unwrap();
/// // eprintln!("{schema:#?}");
/// eprintln!("Wired chunk:\n{columns:#?}");
///
/// let table_out = DataTable::deserialize(table_id, &schema, &columns).unwrap();
/// eprintln!("Table out:\n{table_out}");
/// #
/// # assert_eq!(table_in, table_out);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DataTable {
    /// Auto-generated `TUID`, uniquely identifying this batch of data and keeping track of the
    /// client's wall-clock.
    pub table_id: TableId,

    /// The entire column of `RowId`s.
    ///
    /// Keeps track of the unique identifier for each row that was generated by the clients.
    pub col_row_id: RowIdVec,

    /// All the rows for all the time columns.
    ///
    /// The times are optional since not all rows are guaranteed to have a timestamp for every
    /// single timeline (though it is highly likely to be the case in practice).
    pub col_timelines: BTreeMap<Timeline, TimeOptVec>,

    /// The entire column of [`EntityPath`]s.
    ///
    /// The entity each row relates to, respectively.
    pub col_entity_path: EntityPathVec,

    /// The entire column of `num_instances`.
    ///
    /// Keeps track of the expected number of instances in each row.
    pub col_num_instances: NumInstancesVec,

    /// All the rows for all the component columns.
    ///
    /// The cells are optional since not all rows will have data for every single component
    /// (i.e. the table is sparse).
    pub columns: BTreeMap<ComponentName, DataCellColumn>,
}

impl DataTable {
    /// Creates a new empty table with the given ID.
    pub fn new(table_id: TableId) -> Self {
        Self {
            table_id,
            col_row_id: Default::default(),
            col_timelines: Default::default(),
            col_entity_path: Default::default(),
            col_num_instances: Default::default(),
            columns: Default::default(),
        }
    }

    /// Builds a new `DataTable` from an iterable of [`DataRow`]s.
    pub fn from_rows(table_id: TableId, rows: impl IntoIterator<Item = DataRow>) -> Self {
        crate::profile_function!();

        let rows = rows.into_iter();

        // Explode all rows into columns, and keep track of which components are involved.
        let mut components = IntSet::default();
        #[allow(clippy::type_complexity)]
        let (col_row_id, col_timepoint, col_entity_path, col_num_instances, column): (
            RowIdVec,
            TimePointVec,
            EntityPathVec,
            NumInstancesVec,
            Vec<_>,
        ) = rows
            .map(|row| {
                components.extend(row.component_names());
                let DataRow {
                    row_id,
                    timepoint,
                    entity_path,
                    num_instances,
                    cells,
                } = row;
                (row_id, timepoint, entity_path, num_instances, cells)
            })
            .multiunzip();

        // All time columns.
        let mut col_timelines: BTreeMap<Timeline, TimeOptVec> = BTreeMap::default();
        for (i, timepoint) in col_timepoint.iter().enumerate() {
            for (timeline, time) in timepoint.iter() {
                match col_timelines.entry(*timeline) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry
                            .insert(smallvec::smallvec![None; i])
                            .push(Some(time.as_i64()));
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        let entry = entry.get_mut();
                        entry.push(Some(time.as_i64()));
                    }
                }
            }

            // handle potential sparseness
            for (timeline, col_time) in &mut col_timelines {
                if timepoint.get(timeline).is_none() {
                    col_time.push(None);
                }
            }
        }

        // Pre-allocate all columns (one per component).
        let mut columns = BTreeMap::default();
        for component in components {
            columns.insert(
                component,
                DataCellColumn(smallvec::smallvec![None; column.len()]),
            );
        }

        // Fill all columns (where possible: data is likely sparse).
        for (i, cells) in column.into_iter().enumerate() {
            for cell in cells.0 {
                let component = cell.component_name();
                // NOTE: unwrap cannot fail, all arrays pre-allocated above.
                columns.get_mut(&component).unwrap()[i] = Some(cell);
            }
        }

        Self {
            table_id,
            col_row_id,
            col_timelines,
            col_entity_path,
            col_num_instances,
            columns,
        }
    }
}

impl DataTable {
    #[inline]
    pub fn num_rows(&self) -> u32 {
        self.col_row_id.len() as _
    }

    #[inline]
    pub fn to_rows(&self) -> impl ExactSizeIterator<Item = DataRow> + '_ {
        let num_rows = self.num_rows() as usize;

        let Self {
            table_id: _,
            col_row_id,
            col_timelines,
            col_entity_path,
            col_num_instances,
            columns,
        } = self;

        (0..num_rows).map(move |i| {
            let cells = columns
                .values()
                .filter_map(|rows| rows[i].clone() /* shallow */);

            DataRow::from_cells(
                col_row_id[i],
                TimePoint::from(
                    col_timelines
                        .iter()
                        .filter_map(|(timeline, times)| {
                            times[i].map(|time| (*timeline, time.into()))
                        })
                        .collect::<BTreeMap<_, _>>(),
                ),
                col_entity_path[i].clone(),
                col_num_instances[i],
                cells,
            )
        })
    }

    /// Computes the maximum value for each and every timeline present across this entire table,
    /// and returns the corresponding [`TimePoint`].
    #[inline]
    pub fn timepoint_max(&self) -> TimePoint {
        let mut timepoint = TimePoint::timeless();
        for (timeline, col_time) in &self.col_timelines {
            if let Some(time) = col_time.iter().flatten().max().copied() {
                timepoint.insert(*timeline, time.into());
            }
        }
        timepoint
    }

    /// Compute and cache the total (heap) allocated size of each individual underlying
    /// [`DataCell`].
    /// This does nothing for cells whose size has already been computed and cached before.
    ///
    /// Beware: this is _very_ costly!
    #[inline]
    pub fn compute_all_size_bytes(&mut self) {
        for column in self.columns.values_mut() {
            column.compute_all_size_bytes();
        }
    }
}

impl SizeBytes for DataTable {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            table_id,
            col_row_id,
            col_timelines,
            col_entity_path,
            col_num_instances,
            columns,
        } = self;

        table_id.heap_size_bytes()
            + col_row_id.heap_size_bytes()
            + col_timelines.heap_size_bytes()
            + col_entity_path.heap_size_bytes()
            + col_num_instances.heap_size_bytes()
            + columns.heap_size_bytes()
    }
}

// --- Serialization ---

use arrow2::{
    array::{Array, ListArray, PrimitiveArray},
    bitmap::Bitmap,
    chunk::Chunk,
    datatypes::{DataType, Field, Schema, TimeUnit},
    offset::Offsets,
    types::NativeType,
};
use arrow2_convert::{
    deserialize::TryIntoCollection, field::ArrowField, serialize::ArrowSerialize,
    serialize::TryIntoArrow,
};

// TODO(#1696): Those names should come from the datatypes themselves.

pub const COLUMN_INSERT_ID: &str = "rerun.insert_id";
pub const COLUMN_ROW_ID: &str = "rerun.row_id";
pub const COLUMN_TIMEPOINT: &str = "rerun.timepoint";
pub const COLUMN_ENTITY_PATH: &str = "rerun.entity_path";
pub const COLUMN_NUM_INSTANCES: &str = "rerun.num_instances";

pub const METADATA_KIND: &str = "rerun.kind";
pub const METADATA_KIND_DATA: &str = "data";
pub const METADATA_KIND_CONTROL: &str = "control";
pub const METADATA_KIND_TIME: &str = "time";
pub const METADATA_TABLE_ID: &str = "rerun.table_id";

impl DataTable {
    /// Serializes the entire table into an arrow payload and schema.
    ///
    /// A serialized `DataTable` contains two kinds of columns: control & data.
    ///
    /// * Control columns are those that drive the behavior of the storage systems.
    ///   They are always present, always dense, and always deserialized upon reception by the
    ///   server.
    ///   Internally, time columns are (de)serialized separately from the rest of the control
    ///   columns for efficiency/QOL concerns: that doesn't change the fact that they are control
    ///   columns all the same!
    /// * Data columns are the ones that hold component data.
    ///   They are optional, potentially sparse, and never deserialized on the server-side (not by
    ///   the storage systems, at least).
    pub fn serialize(&self) -> DataTableResult<(Schema, Chunk<Box<dyn Array>>)> {
        crate::profile_function!();

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        {
            let (control_schema, control_columns) = self.serialize_time_columns();
            schema.fields.extend(control_schema.fields);
            schema.metadata.extend(control_schema.metadata);
            columns.extend(control_columns.into_iter());
        }

        {
            let (control_schema, control_columns) = self.serialize_control_columns()?;
            schema.fields.extend(control_schema.fields);
            schema.metadata.extend(control_schema.metadata);
            columns.extend(control_columns.into_iter());
        }

        {
            let (data_schema, data_columns) = self.serialize_data_columns()?;
            schema.fields.extend(data_schema.fields);
            schema.metadata.extend(data_schema.metadata);
            columns.extend(data_columns.into_iter());
        }

        Ok((schema, Chunk::new(columns)))
    }

    /// Serializes all time columns into an arrow payload and schema.
    fn serialize_time_columns(&self) -> (Schema, Vec<Box<dyn Array>>) {
        crate::profile_function!();

        fn serialize_time_column(
            timeline: Timeline,
            times: &TimeOptVec,
        ) -> (Field, Box<dyn Array>) {
            let data = PrimitiveArray::from(times.as_slice()).to(timeline.datatype());

            let field = Field::new(timeline.name().as_str(), data.data_type().clone(), false)
                .with_metadata([(METADATA_KIND.to_owned(), METADATA_KIND_TIME.to_owned())].into());

            (field, data.boxed())
        }

        let Self {
            table_id: _,
            col_row_id: _,
            col_timelines,
            col_entity_path: _,
            col_num_instances: _,
            columns: _,
        } = self;

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        for (timeline, col_time) in col_timelines {
            let (time_field, time_column) = serialize_time_column(*timeline, col_time);
            schema.fields.push(time_field);
            columns.push(time_column);
        }

        (schema, columns)
    }

    /// Serializes all controls columns into an arrow payload and schema.
    ///
    /// Control columns are those that drive the behavior of the storage systems.
    /// They are always present, always dense, and always deserialized upon reception by the
    /// server.
    fn serialize_control_columns(&self) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
        crate::profile_function!();

        let Self {
            table_id,
            col_row_id,
            col_timelines: _,
            col_entity_path,
            col_num_instances,
            columns: _,
        } = self;

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        let (row_id_field, row_id_column) =
            Self::serialize_control_column(COLUMN_ROW_ID, col_row_id)?;
        schema.fields.push(row_id_field);
        columns.push(row_id_column);

        let (entity_path_field, entity_path_column) =
            Self::serialize_control_column(COLUMN_ENTITY_PATH, col_entity_path)?;
        schema.fields.push(entity_path_field);
        columns.push(entity_path_column);

        let (num_instances_field, num_instances_column) = Self::serialize_primitive_column(
            COLUMN_NUM_INSTANCES,
            col_num_instances.as_slice(),
            None,
        )?;
        schema.fields.push(num_instances_field);
        columns.push(num_instances_column);

        schema.metadata = [(METADATA_TABLE_ID.into(), table_id.to_string())].into();

        Ok((schema, columns))
    }

    /// Serializes a single control column: an iterable of dense arrow-like data.
    pub fn serialize_control_column<C: ArrowSerialize + ArrowField<Type = C> + 'static>(
        name: &str,
        values: &[C],
    ) -> DataTableResult<(Field, Box<dyn Array>)> {
        crate::profile_function!();

        /// Transforms an array of unit values into a list of unit arrays.
        ///
        /// * Before: `[C, C, C, C, C, ...]`
        /// * After: `ListArray[ [C], [C], [C], [C], [C], ... ]`
        // NOTE: keeping that one around, just in case.
        #[allow(dead_code)]
        fn unit_values_to_unit_lists(array: Box<dyn Array>) -> Box<dyn Array> {
            let datatype = array.data_type().clone();
            let datatype = ListArray::<i32>::default_datatype(datatype);
            let offsets = Offsets::try_from_lengths(std::iter::repeat(1).take(array.len()))
                .unwrap()
                .into();
            let validity = None;
            ListArray::<i32>::new(datatype, offsets, array, validity).boxed()
        }

        let data: Box<dyn Array> = values.try_into_arrow()?;
        // let data = unit_values_to_unit_lists(data);

        let mut field = Field::new(name, data.data_type().clone(), false)
            .with_metadata([(METADATA_KIND.to_owned(), METADATA_KIND_CONTROL.to_owned())].into());

        if let DataType::Extension(name, _, _) = data.data_type() {
            field
                .metadata
                .extend([("ARROW:extension:name".to_owned(), name.clone())]);
        }

        Ok((field, data))
    }

    /// Serializes a single control column; optimized path for primitive datatypes.
    pub fn serialize_primitive_column<T: NativeType>(
        name: &str,
        values: &[T],
        datatype: Option<DataType>,
    ) -> DataTableResult<(Field, Box<dyn Array>)> {
        crate::profile_function!();

        let data = PrimitiveArray::from_slice(values);

        let datatype = datatype.unwrap_or(data.data_type().clone());
        let data = data.to(datatype.clone()).boxed();

        let mut field = Field::new(name, datatype.clone(), false)
            .with_metadata([(METADATA_KIND.to_owned(), METADATA_KIND_CONTROL.to_owned())].into());

        if let DataType::Extension(name, _, _) = datatype {
            field
                .metadata
                .extend([("ARROW:extension:name".to_owned(), name)]);
        }

        Ok((field, data))
    }

    /// Serializes all data columns into an arrow payload and schema.
    ///
    /// They are optional, potentially sparse, and never deserialized on the server-side (not by
    /// the storage systems, at least).
    fn serialize_data_columns(&self) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
        crate::profile_function!();

        let Self {
            table_id: _,
            col_row_id: _,
            col_timelines: _,
            col_entity_path: _,
            col_num_instances: _,
            columns: table,
        } = self;

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        for (component, rows) in table {
            // If none of the rows have any data, there's nothing to do here
            // TODO(jleibs): would be nice to make serialize_data_column robust to this case
            // but I'm not sure if returning an empty column is the right thing to do there.
            // See: https://github.com/rerun-io/rerun/issues/2005
            if rows.iter().any(|c| c.is_some()) {
                let (field, column) = Self::serialize_data_column(component.as_str(), rows)?;
                schema.fields.push(field);
                columns.push(column);
            }
        }

        Ok((schema, columns))
    }

    /// Serializes a single data column.
    pub fn serialize_data_column(
        name: &str,
        column: &[Option<DataCell>],
    ) -> DataTableResult<(Field, Box<dyn Array>)> {
        crate::profile_function!();

        /// Create a list-array out of a flattened array of cell values.
        ///
        /// * Before: `[C, C, C, C, C, C, C, ...]`
        /// * After: `ListArray[ [[C, C], [C, C, C], None, [C], [C], ...] ]`
        fn data_to_lists(
            column: &[Option<DataCell>],
            data: Box<dyn Array>,
            ext_name: Option<String>,
        ) -> Box<dyn Array> {
            let datatype = data.data_type().clone();

            let field = {
                let mut field = Field::new("item", datatype, true);

                if let Some(name) = ext_name {
                    field
                        .metadata
                        .extend([("ARROW:extension:name".to_owned(), name)]);
                }

                field
            };

            let datatype = DataType::List(Box::new(field));
            let offsets = Offsets::try_from_lengths(column.iter().map(|cell| {
                cell.as_ref()
                    .map_or(0, |cell| cell.num_instances() as usize)
            }))
            // NOTE: cannot fail, `data` has as many instances as `column`
            .unwrap()
            .into();

            #[allow(clippy::from_iter_instead_of_collect)]
            let validity = Bitmap::from_iter(column.iter().map(|cell| cell.is_some()));

            ListArray::<i32>::new(datatype, offsets, data, validity.into()).boxed()
        }

        // TODO(cmc): All we're doing here is allocating and filling a nice contiguous array so
        // our `ListArray`s can compute their indices and for the serializer to work with...
        // In a far enough future, we could imagine having much finer grain control over the
        // serializer and doing all of this at once, bypassing all the mem copies and
        // allocations.

        let cell_refs = column
            .iter()
            .flatten()
            .map(|cell| cell.as_arrow_ref())
            .collect_vec();

        let ext_name = cell_refs.first().and_then(|cell| match cell.data_type() {
            DataType::Extension(name, _, _) => Some(name),
            _ => None,
        });

        // NOTE: Avoid paying for the cost of the concatenation machinery if there's a single
        // row in the column.
        let data = if cell_refs.len() == 1 {
            data_to_lists(column, cell_refs[0].to_boxed(), ext_name.cloned())
        } else {
            // NOTE: This is a column of cells, it shouldn't ever fail to concatenate since
            // they share the same underlying type.
            let data =
                arrow2::compute::concatenate::concatenate(cell_refs.as_slice()).map_err(|err| {
                    re_log::warn_once!("failed to concatenate cells for column {name}");
                    err
                })?;
            data_to_lists(column, data, ext_name.cloned())
        };

        let field = Field::new(name, data.data_type().clone(), false)
            .with_metadata([(METADATA_KIND.to_owned(), METADATA_KIND_DATA.to_owned())].into());

        Ok((field, data))
    }
}

impl DataTable {
    /// Deserializes an entire table from an arrow payload and schema.
    pub fn deserialize(
        table_id: TableId,
        schema: &Schema,
        chunk: &Chunk<Box<dyn Array>>,
    ) -> DataTableResult<Self> {
        crate::profile_function!();

        // --- Time ---

        let col_timelines: DataTableResult<_> = schema
            .fields
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                field.metadata.get(METADATA_KIND).and_then(|kind| {
                    (kind == METADATA_KIND_TIME).then_some((field.name.as_str(), i))
                })
            })
            .map(|(name, index)| {
                chunk
                    .get(index)
                    .ok_or(DataTableError::MissingColumn(name.to_owned()))
                    .and_then(|column| Self::deserialize_time_column(name, &**column))
            })
            .collect();
        let col_timelines = col_timelines?;

        // --- Control ---

        let control_indices: HashMap<&str, usize> = schema
            .fields
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                field.metadata.get(METADATA_KIND).and_then(|kind| {
                    (kind == METADATA_KIND_CONTROL).then_some((field.name.as_str(), i))
                })
            })
            .collect();
        let control_index = move |name: &str| {
            control_indices
                .get(name)
                .copied()
                .ok_or(DataTableError::MissingColumn(name.into()))
        };

        // NOTE: the unwrappings cannot fail since control_index() makes sure the index is valid
        let col_row_id =
            (&**chunk.get(control_index(COLUMN_ROW_ID)?).unwrap()).try_into_collection()?;
        let col_entity_path =
            (&**chunk.get(control_index(COLUMN_ENTITY_PATH)?).unwrap()).try_into_collection()?;
        // TODO(#1712): This is unnecessarily slow...
        let col_num_instances =
            (&**chunk.get(control_index(COLUMN_NUM_INSTANCES)?).unwrap()).try_into_collection()?;

        // --- Components ---

        let columns: DataTableResult<_> = schema
            .fields
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                field.metadata.get(METADATA_KIND).and_then(|kind| {
                    (kind == METADATA_KIND_DATA).then_some((field.name.as_str(), i))
                })
            })
            .map(|(name, index)| {
                let component: ComponentName = name.into();
                chunk
                    .get(index)
                    .ok_or(DataTableError::MissingColumn(name.to_owned()))
                    .and_then(|column| {
                        Self::deserialize_data_column(component, &**column)
                            .map(|data| (component, data))
                    })
            })
            .collect();
        let columns = columns?;

        Ok(Self {
            table_id,
            col_row_id,
            col_timelines,
            col_entity_path,
            col_num_instances,
            columns,
        })
    }

    /// Deserializes a sparse time column.
    fn deserialize_time_column(
        name: &str,
        column: &dyn Array,
    ) -> DataTableResult<(Timeline, TimeOptVec)> {
        crate::profile_function!();

        // See also [`Timeline::datatype`]
        let timeline = match column.data_type().to_logical_type() {
            DataType::Int64 => Timeline::new_sequence(name),
            DataType::Timestamp(TimeUnit::Nanosecond, None) => Timeline::new_temporal(name),
            _ => {
                return Err(DataTableError::NotATimeColumn {
                    name: name.into(),
                    datatype: column.data_type().clone(),
                })
            }
        };

        let col_time = column
            .as_any()
            .downcast_ref::<PrimitiveArray<i64>>()
            // NOTE: cannot fail, datatype checked above
            .unwrap();
        let col_time: TimeOptVec = col_time.into_iter().map(|time| time.copied()).collect();

        Ok((timeline, col_time))
    }

    /// Deserializes a sparse data column.
    fn deserialize_data_column(
        component: ComponentName,
        column: &dyn Array,
    ) -> DataTableResult<DataCellColumn> {
        crate::profile_function!();
        Ok(DataCellColumn(
            column
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .ok_or(DataTableError::NotAColumn(component.to_string()))?
                .iter()
                // TODO(#1805): Schema metadata gets cloned in every single array.
                // This'll become a problem as soon as we enable batching.
                .map(|array| array.map(|values| DataCell::from_arrow(component, values)))
                .collect(),
        ))
    }
}

// ---

impl DataTable {
    /// Deserializes the contents of an [`ArrowMsg`] into a `DataTable`.
    #[inline]
    pub fn from_arrow_msg(msg: &ArrowMsg) -> DataTableResult<Self> {
        let ArrowMsg {
            table_id,
            timepoint_max: _,
            schema,
            chunk,
        } = msg;

        Self::deserialize(*table_id, schema, chunk)
    }

    /// Serializes the contents of a `DataTable` into an [`ArrowMsg`].
    //
    // TODO(#1760): support serializing the cell size itself, so it can be computed on the clients.
    #[inline]
    pub fn to_arrow_msg(&self) -> DataTableResult<ArrowMsg> {
        let timepoint_max = self.timepoint_max();
        let (schema, chunk) = self.serialize()?;

        Ok(ArrowMsg {
            table_id: self.table_id,
            timepoint_max,
            schema,
            chunk,
        })
    }
}

// ---

impl std::fmt::Display for DataTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (schema, columns) = self.serialize().map_err(|err| {
            re_log::error_once!("couldn't display data table: {err}");
            std::fmt::Error
        })?;
        writeln!(f, "DataTable({}):", self.table_id)?;
        re_format::arrow::format_table(
            columns.columns(),
            schema.fields.iter().map(|field| field.name.as_str()),
        )
        .fmt(f)
    }
}

// ---

#[cfg(not(target_arch = "wasm32"))]
impl DataTable {
    /// Crafts a simple but interesting `DataTable`.
    pub fn example(timeless: bool) -> Self {
        use crate::{
            component_types::{ColorRGBA, Label, Point2D},
            Time,
        };

        let table_id = TableId::random();

        let mut tick = 0i64;
        let mut timepoint = |frame_nr: i64| {
            let tp = if timeless {
                TimePoint::timeless()
            } else {
                TimePoint::from([
                    (Timeline::log_time(), Time::now().into()),
                    (Timeline::log_tick(), tick.into()),
                    (Timeline::new_sequence("frame_nr"), frame_nr.into()),
                ])
            };
            tick += 1;
            tp
        };

        let row0 = {
            let num_instances = 2;
            let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
            let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
            let labels: &[Label] = &[];

            DataRow::from_cells3(
                RowId::random(),
                "a",
                timepoint(1),
                num_instances,
                (points, colors, labels),
            )
        };

        let row1 = {
            let num_instances = 0;
            let colors: &[ColorRGBA] = &[];

            DataRow::from_cells1(RowId::random(), "b", timepoint(1), num_instances, colors)
        };

        let row2 = {
            let num_instances = 1;
            let colors: &[_] = &[ColorRGBA::from_rgb(255, 255, 255)];
            let labels: &[_] = &[Label("hey".into())];

            DataRow::from_cells2(
                RowId::random(),
                "c",
                timepoint(2),
                num_instances,
                (colors, labels),
            )
        };

        let mut table = DataTable::from_rows(table_id, [row0, row1, row2]);
        table.compute_all_size_bytes();

        table
    }
}

#[test]
fn data_table_sizes_basics() {
    use crate::Component as _;
    use arrow2::array::{BooleanArray, UInt64Array};

    fn expect(mut cell: DataCell, num_rows: usize, num_bytes: u64) {
        cell.compute_size_bytes();

        let row = DataRow::from_cells1(
            RowId::random(),
            "a/b/c",
            TimePoint::default(),
            cell.num_instances(),
            cell,
        );

        let table = DataTable::from_rows(
            TableId::random(),
            std::iter::repeat_with(|| row.clone()).take(num_rows),
        );
        assert_eq!(num_bytes, table.heap_size_bytes());

        let mut table = DataTable::from_arrow_msg(&table.to_arrow_msg().unwrap()).unwrap();
        table.compute_all_size_bytes();
        let num_bytes = table.heap_size_bytes();
        assert_eq!(num_bytes, table.heap_size_bytes());
    }

    // boolean
    let mut cell = DataCell::from_arrow(
        "some_bools".into(),
        BooleanArray::from(vec![Some(true), Some(false), Some(true)]).boxed(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        2_690_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow("some_bools".into(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        2_690_064, // expected_num_bytes
    );

    // primitive
    let mut cell = DataCell::from_arrow(
        "some_u64s".into(),
        UInt64Array::from_vec(vec![1, 2, 3]).boxed(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        2_840_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow("some_u64s".into(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        2_680_064, // expected_num_bytes
    );

    // utf8 (and more generally: dyn_binary)
    let mut cell = DataCell::from_native(
        [
            crate::component_types::Label("hey".into()),
            crate::component_types::Label("hey".into()),
            crate::component_types::Label("hey".into()),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        3_090_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(
            crate::component_types::Label::name(),
            cell.to_arrow().sliced(1, 1),
        ),
        10_000,    // num_rows
        2_950_064, // expected_num_bytes
    );

    // struct
    let mut cell = DataCell::from_native(
        [
            crate::component_types::Point2D::new(42.0, 666.0),
            crate::component_types::Point2D::new(42.0, 666.0),
            crate::component_types::Point2D::new(42.0, 666.0),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        5_260_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(
            crate::component_types::Point2D::name(),
            cell.to_arrow().sliced(1, 1),
        ),
        10_000,    // num_rows
        5_100_064, // expected_num_bytes
    );

    // struct + fixedsizelist
    let mut cell = DataCell::from_native(
        [
            crate::component_types::Vec2D::from([42.0, 666.0]),
            crate::component_types::Vec2D::from([42.0, 666.0]),
            crate::component_types::Vec2D::from([42.0, 666.0]),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        4_080_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(
            crate::component_types::Point2D::name(),
            cell.to_arrow().sliced(1, 1),
        ),
        10_000,    // num_rows
        3_920_064, // expected_num_bytes
    );

    // variable list
    let mut cell = DataCell::from_native(
        [
            crate::component_types::LineStrip2D::from(vec![
                [42.0, 666.0],
                [42.0, 666.0],
                [42.0, 666.0],
            ]),
            crate::component_types::LineStrip2D::from(vec![
                [42.0, 666.0],
                [42.0, 666.0],
                [42.0, 666.0],
            ]),
            crate::component_types::LineStrip2D::from(vec![
                [42.0, 666.0],
                [42.0, 666.0],
                [42.0, 666.0],
            ]),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        6_120_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(
            crate::component_types::Point2D::name(),
            cell.to_arrow().sliced(1, 1),
        ),
        10_000,    // num_rows
        5_560_064, // expected_num_bytes
    );
}

#[test]
fn data_table_sizes_unions() {
    use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

    fn expect(mut cell: DataCell, num_rows: usize, num_bytes: u64) {
        cell.compute_size_bytes();

        let row = DataRow::from_cells1(
            RowId::random(),
            "a/b/c",
            TimePoint::default(),
            cell.num_instances(),
            cell,
        );

        let table = DataTable::from_rows(
            TableId::random(),
            std::iter::repeat_with(|| row.clone()).take(num_rows),
        );
        assert_eq!(num_bytes, table.heap_size_bytes());

        let err_margin = (num_bytes as f64 * 0.01) as u64;
        let num_bytes_min = num_bytes;
        let num_bytes_max = num_bytes + err_margin;

        let mut table = DataTable::from_arrow_msg(&table.to_arrow_msg().unwrap()).unwrap();
        table.compute_all_size_bytes();
        let num_bytes = table.heap_size_bytes();
        assert!(
            num_bytes_min <= num_bytes && num_bytes <= num_bytes_max,
            "{num_bytes_min} <= {num_bytes} <= {num_bytes_max}"
        );
    }

    // This test uses an artificial enum type to test the union serialization.
    // The transform type does *not* represent our current transform representation.

    // --- Dense ---

    #[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    #[arrow_field(type = "dense")]
    enum DenseTransform {
        Unknown,
        Transform3D(crate::component_types::Transform3DRepr),
        Pinhole(crate::component_types::Pinhole),
    }

    impl crate::Component for DenseTransform {
        #[inline]
        fn name() -> crate::ComponentName {
            "rerun.dense_transform".into()
        }
    }

    // dense union (uniform)
    expect(
        DataCell::from_native(
            [
                DenseTransform::Unknown,
                DenseTransform::Unknown,
                DenseTransform::Unknown,
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        49_030_064, // expected_num_bytes
    );

    // dense union (varying)
    expect(
        DataCell::from_native(
            [
                DenseTransform::Unknown,
                DenseTransform::Transform3D(
                    crate::component_types::TranslationAndMat3 {
                        translation: Some([10.0, 11.0, 12.0].into()),
                        matrix: [[13.0, 14.0, 15.0], [16.0, 17.0, 18.0], [19.0, 20.0, 21.0]].into(),
                    }
                    .into(),
                ),
                DenseTransform::Pinhole(crate::component_types::Pinhole {
                    image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]]
                        .into(),
                    resolution: Some([123.0, 456.0].into()),
                }),
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        49_020_064, // expected_num_bytes
    );

    // --- Sparse ---

    #[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    #[arrow_field(type = "sparse")]
    enum SparseTransform {
        Unknown,
        Pinhole(crate::component_types::Pinhole),
    }

    impl crate::Component for SparseTransform {
        #[inline]
        fn name() -> crate::ComponentName {
            "rerun.sparse_transform".into()
        }
    }

    // sparse union (uniform)
    expect(
        DataCell::from_native(
            [
                SparseTransform::Unknown,
                SparseTransform::Unknown,
                SparseTransform::Unknown,
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        22_180_064, // expected_num_bytes
    );

    // sparse union (varying)
    expect(
        DataCell::from_native(
            [
                SparseTransform::Unknown,
                SparseTransform::Pinhole(crate::component_types::Pinhole {
                    image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]]
                        .into(),
                    resolution: Some([123.0, 456.0].into()),
                }),
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        21_730_064, // expected_num_bytes
    );
}
