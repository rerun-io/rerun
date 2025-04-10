use std::sync::Arc;

use arrow::array::{Float32Array, Int64Array, ListArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};

use re_log_types::TableId;
use re_sorbet::ComponentColumnDescriptor;
use re_types::external::arrow::{
    array::{ArrayRef, RecordBatch},
    datatypes::Schema,
};
use re_types_core::ComponentBatch as _;

#[derive(Default)]
struct SorbetBatchStore {
    batches: Vec<re_sorbet::SorbetBatch>,
}

#[derive(Default)]
pub struct TableStore {
    // Don't ever expose this to the outside world.
    store_engine: parking_lot::RwLock<SorbetBatchStore>,
}

impl TableStore {
    pub fn batches(&self) -> Vec<re_sorbet::SorbetBatch> {
        self.store_engine.read().batches.clone()
    }

    pub fn add_batch(&self, batch: re_sorbet::SorbetBatch) {
        self.store_engine.write().batches.push(batch);
    }

    /// This is just for testing purposes and will go away soon™
    // TODO(grtlr): This is just for debugging purposes until we can populate the
    // store from the outside, for example vie GRPC.
    pub fn dummy() -> Self {
        let mut descriptors = vec![];
        let mut columns = vec![];

        {
            let descriptor = re_sorbet::ColumnDescriptor::Component(ComponentColumnDescriptor {
                entity_path: re_log_types::EntityPath::from("/some/path"),
                archetype_name: Some("archetype".to_owned().into()),
                archetype_field_name: Some("field".to_owned().into()),
                component_name: re_types_core::ComponentName::new("component"),
                store_datatype: arrow::datatypes::DataType::Int64,
                is_static: true,
                is_tombstone: false,
                is_semantically_empty: false,
                is_indicator: false,
            });

            descriptors.push(descriptor);
            columns.push(Arc::new(Int64Array::from(vec![42])) as ArrayRef);
        }

        {
            let field = Arc::new(Field::new("data", DataType::Float32, false));

            let descriptor = re_sorbet::ColumnDescriptor::Component(ComponentColumnDescriptor {
                entity_path: re_log_types::EntityPath::from("/some/path"),
                archetype_name: Some("archetype".to_owned().into()),
                archetype_field_name: Some("short_list".to_owned().into()),
                component_name: re_types_core::ComponentName::new("short_list"),
                store_datatype: arrow::datatypes::DataType::List(field.clone()),
                is_static: true,
                is_tombstone: false,
                is_semantically_empty: false,
                is_indicator: false,
            });

            let data = ListArray::new(
                field,
                OffsetBuffer::from_lengths([5]),
                Arc::new(Float32Array::from(vec![1.0, 2.0, 3.0, 4.0, 5.0])),
                None,
            );

            descriptors.push(descriptor);
            columns.push(Arc::new(data) as ArrayRef);
        }

        {
            let field = Arc::new(Field::new("data", DataType::Float32, false));

            let descriptor = re_sorbet::ColumnDescriptor::Component(ComponentColumnDescriptor {
                entity_path: re_log_types::EntityPath::from("/some/path"),
                archetype_name: Some("archetype".to_owned().into()),
                archetype_field_name: Some("long_list".to_owned().into()),
                component_name: re_types_core::ComponentName::new("long_list"),
                store_datatype: arrow::datatypes::DataType::List(field.clone()),
                is_static: true,
                is_tombstone: false,
                is_semantically_empty: false,
                is_indicator: false,
            });

            let data = ListArray::new(
                field,
                OffsetBuffer::from_lengths([500]),
                Arc::new(Float32Array::from(vec![15.0; 500])),
                None,
            );

            descriptors.push(descriptor);
            columns.push(Arc::new(data) as ArrayRef);
        }

        {
            let blob = re_types::components::Blob(re_types::datatypes::Blob::from(
                re_ui::icons::RERUN_MENU.png_bytes,
            ));

            let array = Arc::new(
                blob.to_arrow_list_array()
                    .expect("serialization should succeed"),
            ) as ArrayRef;

            let descriptor = re_sorbet::ColumnDescriptor::Component(ComponentColumnDescriptor {
                entity_path: re_log_types::EntityPath::from("/some/path"),
                archetype_name: Some("archetype".to_owned().into()),
                archetype_field_name: Some("thumbnail".to_owned().into()),
                component_name: "rerun.components.Blob".into(),
                store_datatype: array.data_type().clone(),
                is_static: true,
                is_tombstone: false,
                is_semantically_empty: false,
                is_indicator: false,
            });

            descriptors.push(descriptor);
            columns.push(array);
        }

        let schema = Arc::new(Schema::new_with_metadata(
            descriptors
                .iter()
                .map(|desc| desc.to_arrow_field(re_sorbet::BatchType::Dataframe))
                .collect::<Vec<_>>(),
            Default::default(),
        ));

        let batch =
            RecordBatch::try_new(schema.clone(), columns).expect("could not create record batch");

        let batch =
            re_sorbet::SorbetBatch::try_from_record_batch(&batch, re_sorbet::BatchType::Dataframe)
                .expect("could not build sorbet batch");

        let store = SorbetBatchStore {
            batches: vec![batch],
        };

        Self {
            store_engine: parking_lot::RwLock::new(store),
        }
    }
}

pub type TableStores = ahash::HashMap<TableId, TableStore>;
