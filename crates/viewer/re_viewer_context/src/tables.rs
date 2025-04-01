use std::sync::Arc;

use arrow::array::Int64Array;
use re_sorbet::ComponentColumnDescriptor;
use re_types::external::arrow::{
    array::{ArrayRef, RecordBatch},
    datatypes::Schema,
};

use crate::{store_hub::StorageContext, StoreBundle, StoreHub};

#[derive(
    Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TableId(Arc<String>);

impl TableId {
    pub fn new(id: String) -> Self {
        Self(Arc::new(id))
    }
}

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for TableId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl std::ops::Deref for TableId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

#[derive(Default)]
struct SorbetBatchStore {
    batches: Vec<re_sorbet::SorbetBatch>,
}

// TODO: remove `Default`
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
}

// TODO(grtlr): This is just for debugging purposes until we can populate the
// store from the outside, for example vie GRPC.
impl Default for TableStore {
    fn default() -> Self {
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

pub struct TableContext {
    /// The current active table.
    pub table_id: TableId,
}
