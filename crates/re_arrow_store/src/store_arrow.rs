use arrow2::{
    array::Array,
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
};
use re_log_types::{
    DataTable, DataTableResult, COLUMN_NUM_INSTANCES, COLUMN_ROW_ID, METADATA_KIND,
    METADATA_KIND_CONTROL,
};

use crate::store::{IndexedBucket, IndexedBucketInner, PersistentIndexedTable};

// ---

// TODO: sort columns

pub const COLUMN_INSERT_ID: &str = "rerun.insert_id";

impl IndexedBucket {
    /// Serializes the entire bucket into an arrow payload and schema.
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

    fn serialize_control_columns(&self) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
        crate::profile_function!();

        let Self {
            timeline: _,
            cluster_key: _,
            inner,
        } = self;

        // TODO
        let (time_field, time_column) = {
            let (name, data) = self.times();

            let mut field = Field::new(name, data.data_type().clone(), false).with_metadata(
                [(METADATA_KIND.to_owned(), METADATA_KIND_CONTROL.to_owned())].into(),
            );

            // TODO(cmc): why do we have to do this manually on the way out, but it's done
            // automatically on our behalf on the way in...?
            if let DataType::Extension(name, _, _) = data.data_type() {
                field
                    .metadata
                    .extend([("ARROW:extension:name".to_owned(), name.clone())]);
            }

            (field, data.boxed())
        };

        let IndexedBucketInner {
            is_sorted: _,
            time_range: _,
            col_time: _,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns: _,
            total_size_bytes: _,
        } = &*inner.read();

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        schema.fields.push(time_field);
        columns.push(time_column);

        let (insert_id_field, insert_id_column) =
            DataTable::serialize_control_column(COLUMN_INSERT_ID, col_insert_id)?;
        schema.fields.push(insert_id_field);
        columns.push(insert_id_column);

        let (row_id_field, row_id_column) =
            DataTable::serialize_control_column(COLUMN_ROW_ID, col_row_id)?;
        schema.fields.push(row_id_field);
        columns.push(row_id_column);

        // TODO(#1712): This is unnecessarily slow...
        let (num_instances_field, num_instances_column) =
            DataTable::serialize_control_column(COLUMN_NUM_INSTANCES, col_num_instances)?;
        schema.fields.push(num_instances_field);
        columns.push(num_instances_column);

        Ok((schema, columns))
    }

    fn serialize_data_columns(&self) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
        crate::profile_function!();

        let Self {
            timeline: _,
            cluster_key: _,
            inner,
        } = self;

        let IndexedBucketInner {
            is_sorted: _,
            time_range: _,
            col_time: _,
            col_insert_id: _,
            col_row_id: _,
            col_num_instances: _,
            columns: table,
            total_size_bytes: _,
        } = &*inner.read();

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        for (component, column) in table {
            let (field, column) = DataTable::serialize_data_column(component.as_str(), column)?;
            schema.fields.push(field);
            columns.push(column);
        }

        Ok((schema, columns))
    }
}

impl PersistentIndexedTable {
    /// Serializes the entire table into an arrow payload and schema.
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

    fn serialize_control_columns(&self) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
        crate::profile_function!();

        let Self {
            ent_path: _,
            cluster_key: _,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns: _,
            total_size_bytes: _,
        } = self;

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        let (insert_id_field, insert_id_column) =
            DataTable::serialize_control_column(COLUMN_INSERT_ID, col_insert_id)?;
        schema.fields.push(insert_id_field);
        columns.push(insert_id_column);

        let (row_id_field, row_id_column) =
            DataTable::serialize_control_column(COLUMN_ROW_ID, col_row_id)?;
        schema.fields.push(row_id_field);
        columns.push(row_id_column);

        // TODO(#1712): This is unnecessarily slow...
        let (num_instances_field, num_instances_column) =
            DataTable::serialize_control_column(COLUMN_NUM_INSTANCES, col_num_instances)?;
        schema.fields.push(num_instances_field);
        columns.push(num_instances_column);

        Ok((schema, columns))
    }

    fn serialize_data_columns(&self) -> DataTableResult<(Schema, Vec<Box<dyn Array>>)> {
        crate::profile_function!();

        let Self {
            ent_path: _,
            cluster_key: _,
            col_insert_id: _,
            col_row_id: _,
            col_num_instances: _,
            columns: table,
            total_size_bytes: _,
        } = self;

        let mut schema = Schema::default();
        let mut columns = Vec::new();

        for (component, column) in table {
            let (field, column) = DataTable::serialize_data_column(component.as_str(), column)?;
            schema.fields.push(field);
            columns.push(column);
        }

        Ok((schema, columns))
    }
}
