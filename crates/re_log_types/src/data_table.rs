use ahash::HashMap;
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};
use smallvec::SmallVec;

use crate::{
    ArrowMsg, ComponentName, DataCell, DataCellError, DataRow, DataRowError, EntityPath, RowId,
    TableId, TimePoint,
};

// ---

#[derive(thiserror::Error, Debug)]
pub enum DataTableError {
    #[error("Trying to deserialize data that is missing a column present in the schema: {0:?}")]
    MissingColumn(String),

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

// TODO(#1757): The timepoint should be serialized as one column per timeline... that would be both
// more efficient and yield much better debugging views of our tables.

// TODO(#1712): implement fast ser/deser paths for primitive types, both control & data.

// ---

pub type RowIdVec = SmallVec<[RowId; 4]>;

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
}

// ---

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
/// ┌───────────────────────┬───────────────────────────────────┬────────────────────┬─────────────────────┬─────────────┬──────────────────────────────────┬─────────────────┐
/// │ rerun.row_id          ┆ rerun.timepoint                   ┆ rerun.entity_path  ┆ rerun.num_instances ┆ rerun.label ┆ rerun.point2d                    ┆ rerun.colorrgba │
/// ╞═══════════════════════╪═══════════════════════════════════╪════════════════════╪═════════════════════╪═════════════╪══════════════════════════════════╪═════════════════╡
/// │ {167967218, 54449486} ┆ [{frame_nr, 1, 1}, {clock, 1, 1}] ┆ a                  ┆ 2                   ┆ []          ┆ [{x: 10, y: 10}, {x: 20, y: 20}] ┆ [2155905279]    │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ {167967218, 54449486} ┆ [{frame_nr, 1, 1}, {clock, 1, 2}] ┆ b                  ┆ 0                   ┆ -           ┆ -                                ┆ []              │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ {167967218, 54449486} ┆ [{frame_nr, 1, 2}, {clock, 1, 1}] ┆ c                  ┆ 1                   ┆ [hey]       ┆ -                                ┆ [4294967295]    │
/// └───────────────────────┴───────────────────────────────────┴────────────────────┴─────────────────────┴─────────────┴──────────────────────────────────┴─────────────────┘
/// ```
///
/// ## Example
///
/// ```rust
/// # use re_log_types::{
/// #     component_types::{ColorRGBA, Label, Point2D, RowId, TableId},
/// #     DataRow, DataTable, Timeline, TimePoint,
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
// TODO(#1619): introduce RowId & TableId
#[derive(Debug, Clone, PartialEq)]
pub struct DataTable {
    /// Auto-generated `TUID`, uniquely identifying this batch of data and keeping track of the
    /// client's wall-clock.
    pub table_id: TableId,

    /// The entire column of `RowId`s.
    ///
    /// Keeps track of the unique identifier for each row that was generated by the clients.
    pub col_row_id: RowIdVec,

    /// The entire column of [`TimePoint`]s.
    pub col_timepoint: TimePointVec,

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
    pub columns: IntMap<ComponentName, DataCellColumn>,
}

impl DataTable {
    /// Creates a new empty table with the given ID.
    pub fn new(table_id: TableId) -> Self {
        Self {
            table_id,
            col_row_id: Default::default(),
            col_timepoint: Default::default(),
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

        // Pre-allocate all columns (one per component).
        let mut columns = IntMap::default();
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

        if col_row_id.len() > 1 {
            re_log::warn_once!(
                "batching features are not ready for use, use single-row data tables instead!"
            );
        }

        Self {
            table_id,
            col_row_id,
            col_timepoint,
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
    pub fn as_rows(&self) -> impl ExactSizeIterator<Item = DataRow> + '_ {
        let num_rows = self.num_rows() as usize;

        let Self {
            table_id: _,
            col_row_id,
            col_timepoint,
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
                col_timepoint[i].clone(),
                col_entity_path[i].clone(),
                col_num_instances[i],
                cells,
            )
        })
    }

    #[inline]
    pub fn into_rows(self) -> impl Iterator<Item = DataRow> {
        let Self {
            table_id: _,
            col_row_id,
            col_timepoint,
            col_entity_path,
            col_num_instances,
            columns,
        } = self;

        use itertools::Itertools as _;

        let col_row_id = col_row_id.into_iter();
        let col_timepoint = col_timepoint.into_iter();
        let col_entity_path = col_entity_path.into_iter();
        let col_num_instances = col_num_instances.into_iter();

        let mut columns = columns
            .into_values()
            .map(|column| column.0.into_iter())
            .collect_vec();
        let rows = std::iter::from_fn(move || {
            let mut next = Vec::with_capacity(columns.len());
            for column in &mut columns {
                if let Some(cell) = column.next()? {
                    next.push(cell);
                }
            }

            Some(next)
        });

        let control = itertools::izip!(
            col_row_id,
            col_timepoint,
            col_entity_path,
            col_num_instances,
            rows,
        );

        control.map(|(row_id, timepoint, entity_path, num_instances, cells)| {
            DataRow::from_cells(row_id, timepoint, entity_path, num_instances, cells)
        })
    }

    /// Computes the maximum value for each and every timeline present across this entire table,
    /// and returns the corresponding [`TimePoint`].
    #[inline]
    pub fn timepoint_max(&self) -> TimePoint {
        self.col_timepoint
            .iter()
            .fold(TimePoint::timeless(), |acc, tp| acc.union_max(tp))
    }
}

// --- Serialization ---

use arrow2::{
    array::{Array, ListArray},
    bitmap::Bitmap,
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
    offset::Offsets,
};
use arrow2_convert::{
    deserialize::TryIntoCollection, field::ArrowField, serialize::ArrowSerialize,
    serialize::TryIntoArrow,
};

// TODO(#1696): Those names should come from the datatypes themselves.

pub const COLUMN_ROW_ID: &str = "rerun.row_id";
pub const COLUMN_TIMEPOINT: &str = "rerun.timepoint";
pub const COLUMN_ENTITY_PATH: &str = "rerun.entity_path";
pub const COLUMN_NUM_INSTANCES: &str = "rerun.num_instances";

pub const METADATA_KIND: &str = "rerun.kind";
pub const METADATA_KIND_DATA: &str = "data";
pub const METADATA_KIND_CONTROL: &str = "control";
pub const METADATA_TABLE_ID: &str = "rerun.table_id";

impl DataTable {
    /// Serializes the entire table into an arrow payload and schema.
    ///
    /// A serialized `DataTable` contains two kinds of columns: control & data.
    ///
    /// * Control columns are those that drive the behavior of the storage systems.
    ///   They are always present, always dense, and always deserialized upon reception by the
    ///   server.
    /// * Data columns are the one that hold component data.
    ///   They are optional, potentially sparse, and never deserialized on the server-side (not by
    ///   the storage systems, at least).
    pub fn serialize(&self) -> DataTableResult<(Schema, Chunk<Box<dyn Array>>)> {
        crate::profile_function!();

        let mut schema = Schema::default();
        let mut columns = Vec::new();

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
            col_timepoint,
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

        let (timepoint_field, timepoint_column) =
            Self::serialize_control_column(COLUMN_TIMEPOINT, col_timepoint)?;
        schema.fields.push(timepoint_field);
        columns.push(timepoint_column);

        let (entity_path_field, entity_path_column) =
            Self::serialize_control_column(COLUMN_ENTITY_PATH, col_entity_path)?;
        schema.fields.push(entity_path_field);
        columns.push(entity_path_column);

        // TODO(#1712): This is unnecessarily slow...
        let (num_instances_field, num_instances_column) =
            Self::serialize_control_column(COLUMN_NUM_INSTANCES, col_num_instances)?;
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

        // TODO(cmc): why do we have to do this manually on the way out, but it's done
        // automatically on our behalf on the way in...?
        if let DataType::Extension(name, _, _) = data.data_type() {
            field
                .metadata
                .extend([("ARROW:extension:name".to_owned(), name.clone())]);
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
            col_timepoint: _,
            col_entity_path: _,
            col_num_instances: _,
            columns: table,
        } = self;

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        for (component, rows) in table {
            let (field, column) = Self::serialize_data_column(component.as_str(), rows)?;
            schema.fields.push(field);
            columns.push(column);
        }

        Ok((schema, columns))
    }

    /// Serializes a single data column.
    pub fn serialize_data_column(
        name: &str,
        column: &[Option<DataCell>],
    ) -> DataTableResult<(Field, Box<dyn Array>)> {
        /// Create a list-array out of a flattened array of cell values.
        ///
        /// * Before: `[C, C, C, C, C, C, C, ...]`
        /// * After: `ListArray[ [[C, C], [C, C, C], None, [C], [C], ...] ]`
        fn data_to_lists(column: &[Option<DataCell>], data: Box<dyn Array>) -> Box<dyn Array> {
            let datatype = data.data_type().clone();

            let datatype = ListArray::<i32>::default_datatype(datatype);
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

        // NOTE: Avoid paying for the cost of the concatenation machinery if there's a single
        // row in the column.
        let data = if cell_refs.len() == 1 {
            data_to_lists(column, cell_refs[0].to_boxed())
        } else {
            // NOTE: This is a column of cells, it shouldn't ever fail to concatenate since
            // they share the same underlying type.
            let data = arrow2::compute::concatenate::concatenate(cell_refs.as_slice())?;
            data_to_lists(column, data)
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
        let col_timepoint =
            (&**chunk.get(control_index(COLUMN_TIMEPOINT)?).unwrap()).try_into_collection()?;
        let col_entity_path =
            (&**chunk.get(control_index(COLUMN_ENTITY_PATH)?).unwrap()).try_into_collection()?;
        // TODO(#1712): This is unnecessarily slow...
        let col_num_instances =
            (&**chunk.get(control_index(COLUMN_NUM_INSTANCES)?).unwrap()).try_into_collection()?;

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
            col_timepoint,
            col_entity_path,
            col_num_instances,
            columns,
        })
    }

    /// Deserializes a sparse data column.
    fn deserialize_data_column(
        component: ComponentName,
        column: &dyn Array,
    ) -> DataTableResult<DataCellColumn> {
        Ok(DataCellColumn(
            column
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .ok_or(DataTableError::NotAColumn(component.to_string()))?
                .iter()
                .map(|array| array.map(|values| DataCell::from_arrow(component, values)))
                .collect(),
        ))
    }
}

// ---

impl TryFrom<&ArrowMsg> for DataTable {
    type Error = DataTableError;

    #[inline]
    fn try_from(msg: &ArrowMsg) -> DataTableResult<Self> {
        let ArrowMsg {
            table_id,
            timepoint_max: _,
            schema,
            chunk,
        } = msg;

        Self::deserialize(*table_id, schema, chunk)
    }
}

impl TryFrom<&DataTable> for ArrowMsg {
    type Error = DataTableError;

    #[inline]
    fn try_from(table: &DataTable) -> DataTableResult<Self> {
        let timepoint_max = table.timepoint_max();
        let (schema, chunk) = table.serialize()?;

        Ok(ArrowMsg {
            table_id: table.table_id,
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
            Time, Timeline,
        };

        let table_id = TableId::random();

        let timepoint = |frame_nr: i64| {
            if timeless {
                TimePoint::timeless()
            } else {
                TimePoint::from([
                    (Timeline::new_temporal("log_time"), Time::now().into()),
                    (Timeline::new_sequence("frame_nr"), frame_nr.into()),
                ])
            }
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

        DataTable::from_rows(table_id, [row0, row1, row2])
    }
}
