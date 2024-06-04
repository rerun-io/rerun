use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use ahash::HashMap;
use itertools::{izip, Itertools as _};
use nohash_hasher::IntSet;

use re_types_core::{ComponentName, Loggable, SizeBytes};

use crate::{
    data_row::DataReadResult, ArrowMsg, DataCell, DataCellError, DataRow, DataRowError, EntityPath,
    RowId, TimePoint, Timeline,
};

// ---

#[derive(thiserror::Error, Debug)]
pub enum DataTableError {
    #[error("The schema has a column {0:?} that is missing in the data")]
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

    #[error("Could not serialize component instances to/from Arrow: {0}")]
    Serialization(#[from] re_types_core::SerializationError),

    #[error("Could not deserialize component instances to/from Arrow: {0}")]
    Deserialization(#[from] re_types_core::DeserializationError),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type DataTableResult<T> = ::std::result::Result<T, DataTableError>;

// ---

pub type RowIdVec = VecDeque<RowId>;

pub type TimeOptVec = VecDeque<Option<i64>>;

pub type TimePointVec = VecDeque<TimePoint>;

pub type ErasedTimeVec = VecDeque<i64>;

pub type EntityPathVec = VecDeque<EntityPath>;

pub type DataCellOptVec = VecDeque<Option<DataCell>>;

/// A column's worth of [`DataCell`]s: a sparse collection of [`DataCell`]s that share the same
/// underlying type and likely point to shared, contiguous memory.
///
/// Each cell in the column corresponds to a different row of the same column.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct DataCellColumn(pub DataCellOptVec);

impl std::ops::Deref for DataCellColumn {
    type Target = VecDeque<Option<DataCell>>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
        Self(vec![None; num_rows].into())
    }

    /// Compute and cache the size of each individual underlying [`DataCell`].
    /// This does nothing for cells whose size has already been computed and cached before.
    ///
    /// Beware: this is _very_ costly!
    #[inline]
    pub fn compute_all_size_bytes(&mut self) {
        re_tracing::profile_function!();
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TableId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TableId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);

    /// Create a new unique [`TableId`] based on the current time.
    #[allow(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        Self(re_tuid::Tuid::new())
    }

    /// Returns the next logical [`TableId`].
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`TableId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0.next())
    }

    /// Returns the `n`-next logical [`TableId`].
    ///
    /// This is equivalent to calling [`TableId::next`] `n` times.
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`TableId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn incremented_by(&self, n: u64) -> Self {
        Self(self.0.incremented_by(n))
    }

    #[inline]
    pub fn from_u128(uid: u128) -> Self {
        Self(re_tuid::Tuid::from_u128(uid))
    }
}

impl SizeBytes for TableId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
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

re_types_core::delegate_arrow_tuid!(TableId as "rerun.controls.TableId");

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
/// cell can contain an arbitrary number of instances:
/// ```text
/// [
///   [[C1, C1, C1], [], [C3], [C4, C4, C4], …],
///   [None, [C2, C2], [], [C4], …],
///   [None, [C2, C2], [], None, …],
///   …
/// ]
/// ```
///
/// Consider this example:
/// ```ignore
/// let row0 = {
///     let points: &[MyPoint] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
///     let colors: &[_] = &[MyColor::from_rgb(128, 128, 128)];
///     let labels: &[Label] = &[];
///     DataRow::from_cells3(RowId::new(), "a", timepoint(1, 1), (points, colors, labels))?
/// };
/// let row1 = {
///     let colors: &[MyColor] = &[];
///     DataRow::from_cells1(RowId::new(), "b", timepoint(1, 2), colors)?
/// };
/// let row2 = {
///     let colors: &[_] = &[MyColor::from_rgb(255, 255, 255)];
///     let labels: &[_] = &[Label("hey".into())];
///     DataRow::from_cells2(RowId::new(), "c", timepoint(2, 1), (colors, labels))?
/// };
/// let table = DataTable::from_rows(table_id, [row0, row1, row2]);
/// ```
///
/// A table has no arrow representation nor datatype of its own, as it is merely a collection of
/// independent rows.
///
/// The table above translates to the following, where each column is contiguous in memory:
/// ```text
/// ┌──────────┬───────────────────────────────┬──────────────────────────────────┬───────────────────┬─────────────┬──────────────────────────────────┬─────────────────┐
/// │ frame_nr ┆ log_time                      ┆ rerun.row_id                     ┆ rerun.entity_path ┆  ┆ rerun.components.Point2D                    ┆ rerun.components.Color │
/// ╞══════════╪═══════════════════════════════╪══════════════════════════════════╪═══════════════════╪═════════════╪══════════════════════════════════╪═════════════════╡
/// │ 1        ┆ 2023-04-05 09:36:47.188796402 ┆ 1753004ACBF5D6E651F2983C3DAF260C ┆ a                 ┆ []          ┆ [{x: 10, y: 10}, {x: 20, y: 20}] ┆ [2155905279]    │
/// ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 1        ┆ 2023-04-05 09:36:47.188852222 ┆ 1753004ACBF5D6E651F2983C3DAF260C ┆ b                 ┆ -           ┆ -                                ┆ []              │
/// ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 2        ┆ 2023-04-05 09:36:47.188855872 ┆ 1753004ACBF5D6E651F2983C3DAF260C ┆ c                 ┆ [hey]       ┆ -                                ┆ [4294967295]    │
/// └──────────┴───────────────────────────────┴──────────────────────────────────┴───────────────────┴─────────────┴──────────────────────────────────┴─────────────────┘
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
            columns: Default::default(),
        }
    }

    /// Builds a new `DataTable` from an iterable of [`DataRow`]s.
    pub fn from_rows(table_id: TableId, rows: impl IntoIterator<Item = DataRow>) -> Self {
        re_tracing::profile_function!();

        let rows = rows.into_iter();

        // Explode all rows into columns, and keep track of which components are involved.
        let mut components = IntSet::default();
        #[allow(clippy::type_complexity)]
        let (col_row_id, col_timepoint, col_entity_path, column): (
            RowIdVec,
            TimePointVec,
            EntityPathVec,
            Vec<_>,
        ) = rows
            .map(|row| {
                components.extend(row.component_names());
                let DataRow {
                    row_id,
                    timepoint,
                    entity_path,
                    cells,
                } = row;
                (row_id, timepoint, entity_path, cells)
            })
            .multiunzip();

        // All time columns.
        let mut col_timelines: BTreeMap<Timeline, TimeOptVec> = BTreeMap::default();
        for (i, timepoint) in col_timepoint.iter().enumerate() {
            for (timeline, time) in timepoint.iter() {
                match col_timelines.entry(*timeline) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry
                            .insert(vec![None; i].into())
                            .push_back(Some(time.as_i64()));
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        let entry = entry.get_mut();
                        entry.push_back(Some(time.as_i64()));
                    }
                }
            }

            // handle potential sparseness
            for (timeline, col_time) in &mut col_timelines {
                if timepoint.get(timeline).is_none() {
                    col_time.push_back(None);
                }
            }
        }

        // Pre-allocate all columns (one per component).
        let mut columns = BTreeMap::default();
        for component in components {
            columns.insert(component, DataCellColumn(vec![None; column.len()].into()));
        }

        // Fill all columns (where possible: data is likely sparse).
        for (i, cells) in column.into_iter().enumerate() {
            for cell in cells.0 {
                let component = cell.component_name();
                // NOTE: unwrap cannot fail, all arrays pre-allocated above.
                #[allow(clippy::unwrap_used)]
                let column = columns.get_mut(&component).unwrap();
                column[i] = Some(cell);
            }
        }

        Self {
            table_id,
            col_row_id,
            col_timelines,
            col_entity_path,
            columns,
        }
    }
}

impl DataTable {
    #[inline]
    pub fn num_rows(&self) -> u32 {
        self.col_row_id.len() as _
    }

    /// Fails if any row has two or more cells share the same component type.
    #[inline]
    pub fn to_rows(&self) -> impl ExactSizeIterator<Item = DataReadResult<DataRow>> + '_ {
        let num_rows = self.num_rows() as usize;

        let Self {
            table_id: _,
            col_row_id,
            col_timelines,
            col_entity_path,
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
                            times[i].map(|time| (*timeline, crate::TimeInt::new_temporal(time)))
                        })
                        .collect::<BTreeMap<_, _>>(),
                ),
                col_entity_path[i].clone(),
                cells,
            )
        })
    }

    /// Computes the maximum value for each and every timeline present across this entire table,
    /// and returns the corresponding [`TimePoint`].
    #[inline]
    pub fn timepoint_max(&self) -> TimePoint {
        let mut timepoint = TimePoint::default();
        for (timeline, col_time) in &self.col_timelines {
            let time = col_time
                .iter()
                .flatten()
                .max()
                .copied()
                .map(crate::TimeInt::new_temporal);

            if let Some(time) = time {
                timepoint.insert(*timeline, time);
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
        re_tracing::profile_function!();
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
            columns,
        } = self;

        table_id.heap_size_bytes()
            + col_row_id.heap_size_bytes()
            + col_timelines.heap_size_bytes()
            + col_entity_path.heap_size_bytes()
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

pub const METADATA_KIND: &str = "rerun.kind";
pub const METADATA_KIND_DATA: &str = "data";
pub const METADATA_KIND_CONTROL: &str = "control";
pub const METADATA_KIND_TIME: &str = "time";

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
        re_tracing::profile_function!();

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        // Temporary compatibility layer with Chunks.
        if let Some(entity_path) = self.col_entity_path.front() {
            /// The key used to identify a Rerun [`EntityPath`] in chunk-level [`ArrowSchema`] metadata.
            //
            // NOTE: Temporarily copied from `re_chunk` while we're transitioning away to the new data
            // model.
            const CHUNK_METADATA_KEY_ENTITY_PATH: &str = "rerun.entity_path";

            schema.metadata.insert(
                CHUNK_METADATA_KEY_ENTITY_PATH.to_owned(),
                entity_path.to_string(),
            );
        }

        {
            let (control_schema, control_columns) = self.serialize_time_columns();
            schema.fields.extend(control_schema.fields);
            schema.metadata.extend(control_schema.metadata);
            columns.extend(control_columns);
        }

        {
            let (control_schema, control_columns) = self.serialize_control_columns()?;
            schema.fields.extend(control_schema.fields);
            schema.metadata.extend(control_schema.metadata);
            columns.extend(control_columns);
        }

        {
            let (data_schema, data_columns) = self.serialize_data_columns()?;
            schema.fields.extend(data_schema.fields);
            schema.metadata.extend(data_schema.metadata);
            columns.extend(data_columns);
        }

        Ok((schema, Chunk::new(columns)))
    }

    /// Serializes all time columns into an arrow payload and schema.
    fn serialize_time_columns(&self) -> (Schema, Vec<Box<dyn Array>>) {
        re_tracing::profile_function!();

        fn serialize_time_column(
            timeline: Timeline,
            times: &TimeOptVec,
        ) -> (Field, Box<dyn Array>) {
            let data = DataTable::serialize_primitive_deque_opt(times).to(timeline.datatype());

            let field = Field::new(timeline.name().as_str(), data.data_type().clone(), false)
                .with_metadata([(METADATA_KIND.to_owned(), METADATA_KIND_TIME.to_owned())].into());

            (field, data.boxed())
        }

        let Self {
            table_id: _,
            col_row_id: _,
            col_timelines,
            col_entity_path: _,
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
        re_tracing::profile_function!();

        let Self {
            table_id,
            col_row_id,
            col_timelines: _,
            col_entity_path,
            columns: _,
        } = self;

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        let (row_id_field, row_id_column) = Self::serialize_control_column(col_row_id)?;
        schema.fields.push(row_id_field);
        columns.push(row_id_column);

        let (entity_path_field, entity_path_column) =
            Self::serialize_control_column(col_entity_path)?;
        schema.fields.push(entity_path_field);
        columns.push(entity_path_column);

        // TODO
        /// The key used to identify a Rerun [`ChunkId`] in chunk-level [`ArrowSchema`] metadata.
        pub const CHUNK_METADATA_KEY_ID: &'static str = "rerun.id";

        schema.metadata = [
            (CHUNK_METADATA_KEY_ID.to_owned(), table_id.to_string()),
            (TableId::name().to_string(), table_id.to_string()),
        ]
        .into();

        Ok((schema, columns))
    }

    /// Serializes a single control column: an iterable of dense arrow-like data.
    pub fn serialize_control_column<'a, C: re_types_core::Component + 'a>(
        values: &'a VecDeque<C>,
    ) -> DataTableResult<(Field, Box<dyn Array>)>
    where
        std::borrow::Cow<'a, C>: std::convert::From<&'a C>,
    {
        re_tracing::profile_function!();

        let data: Box<dyn Array> = C::to_arrow(values)?;

        // TODO(#3360): rethink our extension and metadata usage
        let mut field = C::arrow_field()
            .with_metadata([(METADATA_KIND.to_owned(), METADATA_KIND_CONTROL.to_owned())].into());

        // TODO(#3360): rethink our extension and metadata usage
        if let DataType::Extension(name, _, _) = data.data_type() {
            field
                .metadata
                .extend([("ARROW:extension:name".to_owned(), name.clone())]);
        }

        Ok((field, data))
    }

    /// Serializes a single control column; optimized path for primitive datatypes.
    pub fn serialize_primitive_column<T: arrow2::types::NativeType>(
        name: &str,
        values: &VecDeque<T>,
        datatype: Option<DataType>,
    ) -> (Field, Box<dyn Array>) {
        re_tracing::profile_function!();

        let data = Self::serialize_primitive_deque(values);

        let datatype = datatype.unwrap_or(data.data_type().clone());
        let data = data.to(datatype.clone()).boxed();

        let mut field = Field::new(name, datatype.clone(), false)
            .with_metadata([(METADATA_KIND.to_owned(), METADATA_KIND_CONTROL.to_owned())].into());

        if let DataType::Extension(name, _, _) = datatype {
            field
                .metadata
                .extend([("ARROW:extension:name".to_owned(), name)]);
        }

        (field, data)
    }

    /// Serializes all data columns into an arrow payload and schema.
    ///
    /// They are optional, potentially sparse, and never deserialized on the server-side (not by
    /// the storage systems, at least).
    fn serialize_data_columns(&self) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
        re_tracing::profile_function!();

        let Self {
            table_id: _,
            col_row_id: _,
            col_timelines: _,
            col_entity_path: _,
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
                let (field, column) = Self::serialize_data_column(component, rows)?;
                schema.fields.push(field);
                columns.push(column);
            }
        }

        Ok((schema, columns))
    }

    /// Serializes a single data column.
    pub fn serialize_data_column(
        name: &str,
        column: &VecDeque<Option<DataCell>>,
    ) -> DataTableResult<(Field, Box<dyn Array>)> {
        re_tracing::profile_function!();

        /// Create a list-array out of a flattened array of cell values.
        ///
        /// * Before: `[C, C, C, C, C, C, C, …]`
        /// * After: `ListArray[ [[C, C], [C, C, C], None, [C], [C], …] ]`
        fn data_to_lists(
            column: &VecDeque<Option<DataCell>>,
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

            let datatype = DataType::List(Arc::new(field));
            let offsets = Offsets::try_from_lengths(column.iter().map(|cell| {
                cell.as_ref()
                    .map_or(0, |cell| cell.num_instances() as usize)
            }));
            // NOTE: cannot fail, `data` has as many instances as `column`
            #[allow(clippy::unwrap_used)]
            let offsets = offsets.unwrap().into();

            #[allow(clippy::from_iter_instead_of_collect)]
            let validity = Bitmap::from_iter(column.iter().map(|cell| cell.is_some()));

            ListArray::<i32>::new(datatype, offsets, data, validity.into()).boxed()
        }

        // TODO(cmc): All we're doing here is allocating and filling a nice contiguous array so
        // our `ListArray`s can compute their indices and for the serializer to work with…
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

    pub fn serialize_primitive_deque_opt<T: NativeType>(
        data: &VecDeque<Option<T>>,
    ) -> PrimitiveArray<T> {
        let datatype = T::PRIMITIVE.into();
        let values = data
            .iter()
            .copied()
            .map(Option::unwrap_or_default)
            .collect();
        let validity = data
            .iter()
            .any(Option::is_none)
            .then(|| data.iter().map(Option::is_some).collect());
        PrimitiveArray::new(datatype, values, validity)
    }

    pub fn serialize_primitive_deque<T: NativeType>(data: &VecDeque<T>) -> PrimitiveArray<T> {
        let datatype = T::PRIMITIVE.into();
        let values = data.iter().copied().collect();
        PrimitiveArray::new(datatype, values, None)
    }
}

impl DataTable {
    /// Deserializes an entire table from an arrow payload and schema.
    pub fn deserialize(
        table_id: TableId,
        schema: &Schema,
        chunk: &Chunk<Box<dyn Array>>,
    ) -> DataTableResult<Self> {
        re_tracing::profile_function!();

        /// The key used to identify a Rerun [`EntityPath`] in chunk-level [`ArrowSchema`] metadata.
        //
        // NOTE: Temporarily copied from `re_chunk` while we're transitioning away to the new data
        // model.
        const CHUNK_METADATA_KEY_ENTITY_PATH: &str = "rerun.entity_path";

        let entity_path = schema
            .metadata
            .get(CHUNK_METADATA_KEY_ENTITY_PATH)
            .ok_or_else(|| DataTableError::MissingColumn("metadata:entity_path".to_owned()))?;
        let entity_path = EntityPath::parse_forgiving(entity_path);

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
        #[allow(clippy::unwrap_used)]
        let col_row_id = RowId::from_arrow(
            chunk
                .get(control_index(RowId::name().as_str())?)
                .unwrap()
                .as_ref(),
        )?;
        let col_entity_path = std::iter::repeat_with(|| entity_path.clone())
            .take(col_row_id.len())
            .collect_vec();

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
                let component: ComponentName = name.to_owned().into();
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
            col_row_id: col_row_id.into(),
            col_timelines,
            col_entity_path: col_entity_path.into(),
            columns,
        })
    }

    /// Deserializes a sparse time column.
    fn deserialize_time_column(
        name: &str,
        column: &dyn Array,
    ) -> DataTableResult<(Timeline, TimeOptVec)> {
        re_tracing::profile_function!();

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

        // NOTE: unwrap cannot fail here, datatype checked above
        #[allow(clippy::unwrap_used)]
        let col_time = column
            .as_any()
            .downcast_ref::<PrimitiveArray<i64>>()
            .unwrap();
        let col_time: TimeOptVec = col_time.into_iter().map(|time| time.copied()).collect();

        Ok((timeline, col_time))
    }

    /// Deserializes a sparse data column.
    fn deserialize_data_column(
        component: ComponentName,
        column: &dyn Array,
    ) -> DataTableResult<DataCellColumn> {
        re_tracing::profile_function!();
        Ok(DataCellColumn(
            column
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .ok_or(DataTableError::NotAColumn(component.to_string()))?
                .iter()
                // TODO(#3741): Schema metadata gets cloned in every single array.
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
            on_release: _,
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
            on_release: None,
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
        re_format_arrow::format_dataframe(
            schema.metadata.clone(),
            schema.fields.clone(),
            columns.columns().iter().map(|x| x.as_ref()),
        )
        .fmt(f)
    }
}

impl DataTable {
    /// Checks whether two [`DataTable`]s are _similar_, i.e. not equal on a byte-level but
    /// functionally equivalent.
    ///
    /// Returns `Ok(())` if they match, or an error containing a detailed diff otherwise.
    pub fn similar(table1: &Self, table2: &Self) -> anyhow::Result<()> {
        /// Given a [`DataTable`], returns all of its rows grouped by timeline.
        fn compute_rows(table: &DataTable) -> anyhow::Result<HashMap<Timeline, Vec<DataRow>>> {
            let mut rows_by_timeline: HashMap<Timeline, Vec<DataRow>> = Default::default();

            for row in table.to_rows() {
                let row = row?;
                for (&timeline, &time) in row.timepoint.iter() {
                    let mut row = row.clone();
                    row.timepoint = TimePoint::from([(timeline, time)]);
                    rows_by_timeline.entry(timeline).or_default().push(row);
                }
            }
            Ok(rows_by_timeline)
        }

        let mut rows_by_timeline1 = compute_rows(table1)?;
        let mut rows_by_timeline2 = compute_rows(table2)?;

        for timeline1 in rows_by_timeline1.keys() {
            anyhow::ensure!(
                rows_by_timeline2.contains_key(timeline1),
                "timeline {timeline1:?} was present in the first rrd file but not in the second",
            );
        }
        for timeline2 in rows_by_timeline2.keys() {
            anyhow::ensure!(
                rows_by_timeline1.contains_key(timeline2),
                "timeline {timeline2:?} was present in the second rrd file but not in the first",
            );
        }

        // NOTE: Can't compare `log_time`, by definition.
        rows_by_timeline1.remove(&Timeline::log_time());
        rows_by_timeline2.remove(&Timeline::log_time());

        for (timeline, rows1) in &mut rows_by_timeline1 {
            #[allow(clippy::unwrap_used)] // safe, the keys are checked above
            let rows2 = rows_by_timeline2.get_mut(timeline).unwrap();

            // NOTE: We need both sets of rows to follow a common natural order for the comparison
            // to make sense.
            rows1.sort_by_key(|row| (row.timepoint.clone(), row.row_id));
            rows2.sort_by_key(|row| (row.timepoint.clone(), row.row_id));

            anyhow::ensure!(
                rows1.len() == rows2.len(),
                "rrd files yielded different number of datastore rows for timeline {timeline:?}: {} vs. {}",
                rows1.len(),
                rows2.len()
            );

            for (ri, (row1, row2)) in rows1.iter().zip(rows2).enumerate() {
                let DataRow {
                    row_id: _,
                    timepoint: timepoint1,
                    entity_path: entity_path1,
                    cells: ref cells1,
                } = row1;
                let DataRow {
                    row_id: _,
                    timepoint: timepoint2,
                    entity_path: entity_path2,
                    cells: ref cells2,
                } = row2;

                for (c1, c2) in izip!(&cells1.0, &cells2.0) {
                    if c1 != c2 {
                        anyhow::ensure!(
                            c1.datatype() == c2.datatype(),
                            "Found discrepancy in row #{ri}: cells' datatypes don't match!\n{}",
                            similar_asserts::SimpleDiff::from_str(
                                &format!("{:?}:{:?}", c1.component_name(), c1.datatype()),
                                &format!("{:?}:{:?}", c2.component_name(), c2.datatype()),
                                "cell1",
                                "cell2"
                            )
                        );

                        let arr1 = c1.as_arrow_ref();
                        let arr2 = c2.as_arrow_ref();

                        if let (Some(arr1), Some(arr2)) = (
                            arr1.as_any().downcast_ref::<arrow2::array::UnionArray>(),
                            arr2.as_any().downcast_ref::<arrow2::array::UnionArray>(),
                        ) {
                            anyhow::ensure!(
                                arr1.validity() == arr2.validity(),
                                "Found discrepancy in row #{ri}: union arrays' validity bitmaps don't match!\n{}\n{}",
                                similar_asserts::SimpleDiff::from_str(&row1.to_string(), &row2.to_string(), "row1", "row2"),
                                similar_asserts::SimpleDiff::from_str(
                                    &format!("{:?}", arr1.validity()),
                                    &format!("{:?}", arr2.validity()),
                                    "cell1",
                                    "cell2"
                                )
                            );
                            anyhow::ensure!(
                                arr1.types() == arr2.types(),
                                "Found discrepancy in row #{ri}: union arrays' type indices don't match!\n{}\n{}",
                                similar_asserts::SimpleDiff::from_str(&row1.to_string(), &row2.to_string(), "row1", "row2"),
                                similar_asserts::SimpleDiff::from_str(
                                    &format!("{:?}", arr1.types()),
                                    &format!("{:?}", arr2.types()),
                                    "cell1",
                                    "cell2"
                                )
                            );
                            anyhow::ensure!(
                                arr1.offsets() == arr2.offsets(),
                                "Found discrepancy in row #{ri}: union arrays' offsets don't match!\n{}\n{}",
                                similar_asserts::SimpleDiff::from_str(&row1.to_string(), &row2.to_string(), "row1", "row2"),
                                similar_asserts::SimpleDiff::from_str(
                                    &format!("{:?}", arr1.offsets()),
                                    &format!("{:?}", arr2.offsets()),
                                    "cell1",
                                    "cell2"
                                )
                            );
                        }
                    }
                }

                let mut size_mismatches = vec![];
                for (c1, c2) in izip!(&cells1.0, &cells2.0) {
                    if c1.total_size_bytes() != c2.total_size_bytes() {
                        size_mismatches.push(format!(
                            "Sizes don't match! {} ({}) vs. {} ({}) bytes. Perhaps the validity differs?",
                            c1.total_size_bytes(),
                            c1.component_name(),
                            c2.total_size_bytes(),
                            c2.component_name(),
                        ));

                        fn cell_to_bytes(cell: DataCell) -> anyhow::Result<Vec<u8>> {
                            let row = DataRow::from_cells1(
                                RowId::ZERO,
                                "cell",
                                TimePoint::default(),
                                cell,
                            )?;
                            let table = DataTable::from_rows(TableId::ZERO, [row]);

                            let msg = table.to_arrow_msg()?;

                            use arrow2::io::ipc::write::StreamWriter;
                            let mut buf = Vec::<u8>::new();
                            let mut writer = StreamWriter::new(&mut buf, Default::default());
                            writer.start(&msg.schema, None)?;
                            writer.write(&msg.chunk, None)?;
                            writer.finish()?;

                            Ok(buf)
                        }

                        let c1_bytes = cell_to_bytes(c1.clone())?;
                        let c2_bytes = cell_to_bytes(c2.clone())?;

                        size_mismatches.push(format!(
                            "IPC size is {} vs {} bytes",
                            c1_bytes.len(),
                            c2_bytes.len()
                        ));

                        if c1_bytes.len().max(c2_bytes.len()) < 300 {
                            size_mismatches.push(
                                similar_asserts::SimpleDiff::from_str(
                                    &format!("{c1_bytes:#?}"),
                                    &format!("{c2_bytes:#?}"),
                                    "cell1_ipc",
                                    "cell2_ipc",
                                )
                                .to_string(),
                            );
                        }
                    }
                }

                anyhow::ensure!(
                    timepoint1 == timepoint2 && entity_path1 == entity_path2 && cells1 == cells2,
                    "Found discrepancy in row #{ri}:\n{}\n{}\
                    \n\nrow1:\n{row1}
                    \n\nrow2:\n{row2}",
                    similar_asserts::SimpleDiff::from_str(
                        &row1.to_string(),
                        &row2.to_string(),
                        "row1",
                        "row2"
                    ),
                    size_mismatches.join("\n"),
                );
            }
        }

        Ok(())
    }
}

// ---

/// Crafts a simple but interesting [`DataTable`].
#[cfg(not(target_arch = "wasm32"))]
impl DataTable {
    pub fn example(timeless: bool) -> Self {
        // NOTE: because everything here is predetermined and there is no input we assume it's safe here
        #![allow(clippy::unwrap_used)]
        use crate::{
            example_components::{MyColor, MyLabel, MyPoint},
            Time,
        };

        let table_id = TableId::new();

        let mut tick = 0i64;
        let mut timepoint = |frame_nr: i64| {
            let mut tp = TimePoint::default();
            if !timeless {
                tp.insert(Timeline::log_time(), Time::now());
                tp.insert(Timeline::log_tick(), tick);
                tp.insert(Timeline::new_sequence("frame_nr"), frame_nr);
            }
            tick += 1;
            tp
        };

        let row0 = {
            let positions: &[MyPoint] = &[MyPoint::new(10.0, 10.0), MyPoint::new(20.0, 20.0)];
            let colors: &[_] = &[MyColor(0x8080_80FF)];
            let labels: &[MyLabel] = &[];

            DataRow::from_cells3(RowId::new(), "a", timepoint(1), (positions, colors, labels))
                .unwrap()
        };

        let row1 = {
            let colors: &[MyColor] = &[];

            DataRow::from_cells1(RowId::new(), "b", timepoint(1), colors).unwrap()
        };

        let row2 = {
            let colors: &[_] = &[MyColor(0xFFFF_FFFF)];
            let labels: &[_] = &[MyLabel("hey".into())];

            DataRow::from_cells2(RowId::new(), "c", timepoint(2), (colors, labels)).unwrap()
        };

        let mut table = Self::from_rows(table_id, [row0, row1, row2]);
        table.compute_all_size_bytes();

        table
    }
}
