use std::collections::{BTreeMap, VecDeque};

use arrow2::{array::Array, chunk::Chunk, datatypes::Schema};
use nohash_hasher::IntMap;
use re_log_types::{DataCellColumn, DataTable, DataTableResult, RowId, Timeline};
use re_types_core::ComponentName;

use crate::{
    store::{IndexedBucket, IndexedBucketInner},
    StaticTable,
};

// ---

impl IndexedBucket {
    /// Serializes the entire bucket into an arrow payload and schema.
    ///
    /// Column order:
    /// - `insert_id`
    /// - `row_id`
    /// - `time`
    /// - rest of component columns in ascending lexical order
    pub fn serialize(&self) -> DataTableResult<(Schema, Chunk<Box<dyn Array>>)> {
        re_tracing::profile_function!();

        let Self { timeline, inner } = self;

        let IndexedBucketInner {
            is_sorted: _,
            time_range: _,
            col_time,
            col_insert_id,
            col_row_id,
            max_row_id: _,
            columns,
            size_bytes: _,
        } = &*inner.read();

        serialize(
            Some((*timeline, col_time)),
            col_insert_id,
            col_row_id,
            columns,
        )
    }
}

impl StaticTable {
    /// Serializes the entire table into an arrow payload and schema.
    ///
    /// Column order:
    /// - `insert_id`
    /// - `row_id`
    /// - `time`
    /// - rest of component columns in ascending lexical order
    pub fn serialize(&self) -> DataTableResult<(Schema, Chunk<Box<dyn Array>>)> {
        re_tracing::profile_function!();

        let mut cells_per_row_id: BTreeMap<RowId, Vec<_>> = Default::default();
        for static_cell in self.cells.values() {
            cells_per_row_id
                .entry(static_cell.row_id)
                .or_default()
                .push(static_cell.clone());
        }

        let col_insert_id = cells_per_row_id
            .values()
            .filter_map(|cells| cells.first().and_then(|cell| cell.insert_id))
            .collect();

        let col_row_id = cells_per_row_id.keys().copied().collect();

        let component_names: Vec<_> = self
            .cells
            .values()
            .map(|cell| cell.cell.component_name())
            .collect();

        let mut columns = IntMap::<ComponentName, DataCellColumn>::default();
        for (_row_id, cells) in cells_per_row_id {
            let cells: BTreeMap<_, _> = cells
                .iter()
                .map(|cell| (cell.cell.component_name(), &cell.cell))
                .collect();
            for component_name in &component_names {
                columns
                    .entry(*component_name)
                    .or_default()
                    .push_back(cells.get(component_name).copied().cloned());
            }
        }

        serialize(None, &col_insert_id, &col_row_id, &columns)
    }
}

// ---

fn serialize(
    col_time: Option<(Timeline, &VecDeque<i64>)>,
    col_insert_id: &VecDeque<u64>,
    col_row_id: &VecDeque<RowId>,
    table: &IntMap<ComponentName, DataCellColumn>,
) -> DataTableResult<(Schema, Chunk<Box<dyn Array>>)> {
    re_tracing::profile_function!();

    let mut schema = Schema::default();
    let mut columns = Vec::new();

    // NOTE: Empty table / bucket.
    if col_row_id.is_empty() {
        return Ok((schema, Chunk::new(columns)));
    }

    {
        let (control_schema, control_columns) =
            serialize_control_columns(col_time, col_insert_id, col_row_id)?;
        schema.fields.extend(control_schema.fields);
        schema.metadata.extend(control_schema.metadata);
        columns.extend(control_columns);
    }

    {
        let (data_schema, data_columns) = serialize_data_columns(table)?;
        schema.fields.extend(data_schema.fields);
        schema.metadata.extend(data_schema.metadata);
        columns.extend(data_columns);
    }

    Ok((schema, Chunk::new(columns)))
}

fn serialize_control_columns(
    col_time: Option<(Timeline, &VecDeque<i64>)>,
    col_insert_id: &VecDeque<u64>,
    col_row_id: &VecDeque<RowId>,
) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
    re_tracing::profile_function!();

    let mut schema = Schema::default();
    let mut columns = Vec::new();

    // NOTE: ordering is taken into account!
    // - insert_id
    // - row_id
    // - time

    // NOTE: Optional column, so make sure it's actually there:
    if !col_insert_id.is_empty() {
        let (insert_id_field, insert_id_column) = DataTable::serialize_primitive_column(
            &crate::DataStore::insert_id_component_name(),
            col_insert_id,
            None,
        );
        schema.fields.push(insert_id_field);
        columns.push(insert_id_column);
    }

    let (row_id_field, row_id_column) = DataTable::serialize_control_column(col_row_id)?;
    schema.fields.push(row_id_field);
    columns.push(row_id_column);

    if let Some((timeline, col_time)) = col_time {
        let (time_field, time_column) = DataTable::serialize_primitive_column(
            timeline.name(),
            col_time,
            timeline.datatype().into(),
        );
        schema.fields.push(time_field);
        columns.push(time_column);
    }

    Ok((schema, columns))
}

fn serialize_data_columns(
    table: &IntMap<ComponentName, DataCellColumn>,
) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
    re_tracing::profile_function!();

    let mut schema = Schema::default();
    let mut columns = Vec::new();

    // NOTE: ordering is taken into account!
    let table: BTreeMap<_, _> = table.iter().collect();

    for (component, column) in table {
        // NOTE: Don't serialize columns with only null values.
        if column.iter().any(Option::is_some) {
            let (field, column) = DataTable::serialize_data_column(component, column)?;
            schema.fields.push(field);
            columns.push(column);
        }
    }

    Ok((schema, columns))
}
