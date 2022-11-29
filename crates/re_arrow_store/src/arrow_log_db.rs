use arrow2::array::{Array, ListArray, StructArray};
use arrow2::buffer::Buffer;
use nohash_hasher::IntMap;
use polars::export::arrow::io::ipc::read::{read_stream_metadata, StreamReader, StreamState};
use polars::prelude::*;
use re_log_types::arrow::{filter_time_cols, OBJPATH_KEY};
use re_log_types::{ArrowMsg, FieldName, ObjPath};

pub struct LogDb {
    objects: IntMap<ObjPath, IntMap<FieldName, DataFrame>>,
    /// Registry of known/accepted field types
    field_schema_registry: IntMap<FieldName, Field>,
}

impl Default for LogDb {
    fn default() -> Self {
        let field_schema_registry = IntMap::from_iter([
            (
                FieldName::from("rect"),
                Field::new(
                    "rect",
                    DataType::Struct(vec![
                        Field::new("x", DataType::Float32),
                        Field::new("y", DataType::Float32),
                        Field::new("w", DataType::Float32),
                        Field::new("h", DataType::Float32),
                    ]),
                ),
            ),
            (
                FieldName::from("color_rgba"),
                Field::new("color_rgba", DataType::UInt32),
            ),
            (
                FieldName::from("pos2d"),
                Field::new(
                    "pos2d",
                    DataType::Struct(vec![
                        Field::new("x", DataType::Float32),
                        Field::new("y", DataType::Float32),
                    ]),
                ),
            ),
            (
                FieldName::from("pos3d"),
                Field::new(
                    "pos3d",
                    DataType::Struct(vec![
                        Field::new("x", DataType::Float32),
                        Field::new("y", DataType::Float32),
                        Field::new("z", DataType::Float32),
                    ]),
                ),
            ),
        ]);
        Self {
            objects: Default::default(),
            field_schema_registry,
        }
    }
}

impl LogDb {
    pub fn push_field_data(
        &mut self,
        obj_path: &ObjPath,
        field: &ArrowField,
        col: Box<dyn Array>,
        time_cols: &[Series],
    ) -> Result<(), PolarsError> {
        if !self
            .field_schema_registry
            .contains_key(&FieldName::from(field.name.as_str()))
        {
            return Err(PolarsError::SchemaMisMatch(
                format!(
                    "Unrecognized field logged to '{obj_path}': {}. Ignoring.",
                    field.name
                )
                .into(),
            ));
        }

        // Re-form the input array as a ListArray
        let col = ListArray::try_new(
            ListArray::<i32>::default_datatype(col.data_type().clone()), // datatype
            Buffer::from(vec![0, col.len() as i32]),                     // offsets
            col,                                                         // values
            None,                                                        // validity
        )?;

        let series = Series::try_from((field.name.as_str(), col.boxed()))?;

        let mut all_fields: Vec<Series> = time_cols.into();
        all_fields.push(series);
        let df_new = DataFrame::new(all_fields)?;

        self.objects
            .entry(obj_path.clone())
            .or_default()
            .entry(FieldName::new(field.name.as_str()))
            .and_modify(|df_existing| {
                df_existing.extend(&df_new).unwrap();
            })
            .or_insert(df_new);
        Ok(())
    }

    pub fn push_new_columns(
        &mut self,
        obj_path: &ObjPath,
        schema: &ArrowSchema,
        columns: &[Box<dyn Array>],
    ) -> Result<(), PolarsError> {
        // Outer schema columns for timelines
        let time_cols = filter_time_cols(&schema.fields, columns)
            .map(|(field, col)| Series::try_from((field.name.as_str(), col.clone())))
            .collect::<Result<Vec<_>, _>>()?;

        // Outer schema column representing component fields
        let comps = schema
            .index_of("components")
            .and_then(|idx| columns.get(idx))
            .ok_or_else(|| PolarsError::NotFound("Missing expected 'components' column.".into()))?;

        // Cast to a ListArray
        let comps_list = comps
            .as_any()
            .downcast_ref::<ListArray<i32>>()
            .ok_or_else(|| PolarsError::SchemaMisMatch("Expected ListArray".into()))?;

        // The values of the ListArray should be a StructArray of Rerun components
        let struct_array = comps_list
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .expect("shouldn't fail");

        for (field, col) in struct_array
            .fields()
            .iter()
            .zip(struct_array.values().iter().cloned())
        {
            self.push_field_data(obj_path, field, col, time_cols.as_slice())?;
        }

        Ok(())
    }

    pub fn consume_msg(&mut self, msg: ArrowMsg) {
        let ArrowMsg { msg_id: _, data } = msg;

        if std::env::var("ARROW_DUMP").is_ok() {
            static CNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = &format!("data{}", CNT.load(std::sync::atomic::Ordering::Relaxed));
            re_log::info!("Dumping received Arrow stream to {path:?}");
            std::fs::write(path, data.as_slice()).unwrap();
            CNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        let mut cursor = std::io::Cursor::new(&data);
        let metadata = read_stream_metadata(&mut cursor).unwrap();
        let stream = StreamReader::new(cursor, metadata, None);
        self.consume_stream(stream);
    }

    pub fn consume_stream(&mut self, stream: StreamReader<impl std::io::Read>) {
        let arrow_schema = stream.metadata().schema.clone();

        // Get the object path from the metadata
        let obj_path = arrow_schema
            .metadata
            .get(OBJPATH_KEY)
            .map(|path| ObjPath::from(path.as_str()))
            .expect("Bad ObjPath");

        for item in stream {
            if let StreamState::Some(chunk) = item.unwrap() {
                self.push_new_columns(&obj_path, &arrow_schema, chunk.columns())
                    .unwrap();
            }
        }
    }

    pub fn debug_object_contents(&self) {
        for (path, fields) in &self.objects {
            println!(
                "Object: {path} Keys {:?}",
                fields.keys().collect::<Vec<_>>()
            );

            for field in fields.values() {
                println!("{field:#?}");
            }
        }
    }
}

#[test]
fn tester() {
    let mut logdb = LogDb::default();
    for path in [
        "/Users/john/Source/rerun/data0",
        "/Users/john/Source/rerun/data1",
    ] {
        let mut file = std::fs::File::open(path).unwrap();
        let metadata = read_stream_metadata(&mut file).unwrap();
        let stream = StreamReader::new(file, metadata, None);
        logdb.consume_stream(stream);
    }
    logdb.debug_object_contents();
}
