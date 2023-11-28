use std::collections::{BTreeMap, VecDeque};

use arrow2::{array::Array, chunk::Chunk, datatypes::Schema};
use nohash_hasher::IntMap;
use re_log_types::{DataCellColumn, DataTable, DataTableResult, NumInstances, RowId, Timeline};
use re_types_core::ComponentName;

use crate::store::{
    IndexedBucket, IndexedBucketInner, PersistentIndexedTable, PersistentIndexedTableInner,
};

// ---

impl IndexedBucket {
    /// Serializes the entire bucket into an arrow payload and schema.
    ///
    /// Column order:
    /// - `insert_id`
    /// - `row_id`
    /// - `time`
    /// - `num_instances`
    /// - `$cluster_key`
    /// - rest of component columns in ascending lexical order
    pub fn serialize(&self) -> DataTableResult<(Schema, Chunk<Box<dyn Array>>)> {
        re_tracing::profile_function!();

        let Self {
            timeline,
            cluster_key,
            inner,
        } = self;

        let IndexedBucketInner {
            is_sorted: _,
            time_range: _,
            col_time,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            size_bytes: _,
        } = &*inner.read();

        serialize(
            cluster_key,
            Some((*timeline, col_time)),
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
        )
    }
}

impl PersistentIndexedTable {
    /// Serializes the entire table into an arrow payload and schema.
    ///
    /// Column order:
    /// - `insert_id`
    /// - `row_id`
    /// - `time`
    /// - `num_instances`
    /// - `$cluster_key`
    /// - rest of component columns in ascending lexical order
    pub fn serialize(&self) -> DataTableResult<(Schema, Chunk<Box<dyn Array>>)> {
        re_tracing::profile_function!();

        let Self {
            ent_path: _,
            cluster_key,
            inner,
        } = self;

        let PersistentIndexedTableInner {
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            is_sorted: _,
        } = &*inner.read();

        serialize(
            cluster_key,
            None,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
        )
    }
}

// ---

fn serialize(
    cluster_key: &ComponentName,
    col_time: Option<(Timeline, &VecDeque<i64>)>,
    col_insert_id: &VecDeque<u64>,
    col_row_id: &VecDeque<RowId>,
    col_num_instances: &VecDeque<NumInstances>,
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
            serialize_control_columns(col_time, col_insert_id, col_row_id, col_num_instances)?;
        schema.fields.extend(control_schema.fields);
        schema.metadata.extend(control_schema.metadata);
        columns.extend(control_columns);
    }

    {
        let (data_schema, data_columns) = serialize_data_columns(cluster_key, table)?;
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
    col_num_instances: &VecDeque<NumInstances>,
) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
    re_tracing::profile_function!();

    let mut schema = Schema::default();
    let mut columns = Vec::new();

    // NOTE: ordering is taken into account!
    // - insert_id
    // - row_id
    // - time
    // - num_instances

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

    let (num_instances_field, num_instances_column) =
        DataTable::serialize_control_column(col_num_instances)?;
    schema.fields.push(num_instances_field);
    columns.push(num_instances_column);

    Ok((schema, columns))
}

fn serialize_data_columns(
    cluster_key: &ComponentName,
    table: &IntMap<ComponentName, DataCellColumn>,
) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
    re_tracing::profile_function!();

    let mut schema = Schema::default();
    let mut columns = Vec::new();

    // NOTE: ordering is taken into account!
    let mut table: BTreeMap<_, _> = table.iter().collect();

    // Cluster column first and foremost!
    //
    // NOTE: cannot fail, the cluster key _has_ to be there by definition
    let cluster_column = table.remove(&cluster_key).unwrap();
    {
        let (field, column) = DataTable::serialize_data_column(cluster_key, cluster_column)?;
        schema.fields.push(field);
        columns.push(column);
    }

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
