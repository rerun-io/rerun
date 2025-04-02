use std::sync::Arc;

use arrow::array::Int64Array;
use re_log_types::TableId;
use re_sorbet::ComponentColumnDescriptor;
use re_types::external::arrow::{
    array::{ArrayRef, RecordBatch},
    datatypes::Schema,
};

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
        // TODO: avoid clone
        self.store_engine.read().batches.clone()
    }

    pub fn add_batch(&self, batch: re_sorbet::SorbetBatch) {
        self.store_engine.write().batches.push(batch);
    }

    /// This is just for testing purposes and will go away soon™
    // TODO(grtlr): This is just for debugging purposes until we can populate the
    // store from the outside, for example vie GRPC.
    pub fn dummy() -> Self {
        let descriptor = re_sorbet::ColumnDescriptor::Component(ComponentColumnDescriptor {
            entity_path: re_log_types::EntityPath::from("/some/path"),
            archetype_name: Some("archetype".to_owned().into()),
            archetype_field_name: Some("field".to_owned().into()),
            component_name: re_types_core::ComponentName::new("component"),
            store_datatype: arrow::datatypes::DataType::Int64,
            is_static: true,
            is_tombstone: false,
            is_semantically_empty: false,
            is_indicator: true,
        });

        #[expect(clippy::disallowed_methods)]
        let schema = Arc::new(Schema::new(vec![
            descriptor.to_arrow_field(re_sorbet::BatchType::Dataframe)
        ]));

        let column = vec![Arc::new(Int64Array::from(vec![42])) as ArrayRef];

        let batch =
            RecordBatch::try_new(schema.clone(), column).expect("could not create record batch");

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
