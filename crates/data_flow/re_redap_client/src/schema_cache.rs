use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::datatypes::Schema as ArrowSchema;
use re_protos::common::v1alpha1::ext::SchemaId;

/// Layer schemas already fetched from one server, keyed by their [`SchemaId`].
///
/// A [`SchemaId`] is the hash of the schema it identifies, so entries never go stale and hold for
/// every dataset of the server.
#[derive(Clone, Default)]
pub struct SchemaCacheHandle(Arc<re_mutex::RwLock<SchemaCache>>);

#[derive(Default, re_byte_size::SizeBytes)]
struct SchemaCache {
    schemas: ahash::HashMap<SchemaId, Arc<ArrowSchema>>,

    /// The server answered `/GetSchemas` with `Unimplemented`.
    ///
    /// Not reset by [`SchemaCacheHandle::purge_memory`].
    get_schemas_unimplemented: bool,
}

impl re_byte_size::SizeBytes for SchemaCacheHandle {
    fn heap_size_bytes(&self) -> u64 {
        let Self(cache) = self;

        cache.read().heap_size_bytes()
    }
}

impl SchemaCacheHandle {
    /// Splits `schema_ids` into the schemas this cache holds and the IDs still to be fetched.
    pub fn split_cached(
        &self,
        schema_ids: &BTreeSet<SchemaId>,
    ) -> (BTreeMap<SchemaId, Arc<ArrowSchema>>, BTreeSet<SchemaId>) {
        let cache = self.0.read();

        let mut cached = BTreeMap::new();
        let mut missing = BTreeSet::new();
        for schema_id in schema_ids {
            match cache.schemas.get(schema_id) {
                Some(schema) => {
                    cached.insert(*schema_id, Arc::clone(schema));
                }
                None => {
                    missing.insert(*schema_id);
                }
            }
        }

        (cached, missing)
    }

    pub fn insert(&self, schemas: &BTreeMap<SchemaId, Arc<ArrowSchema>>) {
        let mut cache = self.0.write();
        for (schema_id, schema) in schemas {
            cache
                .schemas
                .entry(*schema_id)
                .or_insert_with(|| Arc::clone(schema));
        }
    }

    pub fn get_schemas_unimplemented(&self) -> bool {
        self.0.read().get_schemas_unimplemented
    }

    pub fn set_get_schemas_unimplemented(&self) {
        self.0.write().get_schemas_unimplemented = true;
    }

    /// Drops all cached schemas.
    pub fn purge_memory(&self) {
        self.0.write().schemas = Default::default();
    }
}

#[cfg(test)]
mod tests {
    use arrow::datatypes::{DataType, Field};

    use super::*;

    fn schema_id(byte: u8) -> SchemaId {
        SchemaId::from_sha256([byte; 32])
    }

    fn schema(name: &str) -> Arc<ArrowSchema> {
        Arc::new(ArrowSchema::new_with_metadata(
            vec![Field::new(name, DataType::Int64, true)],
            Default::default(),
        ))
    }

    #[test]
    fn split_cached_serves_only_inserted_ids() {
        let cache = SchemaCacheHandle::default();
        cache.insert(&BTreeMap::from([(schema_id(1), schema("a"))]));

        let (cached, missing) = cache.split_cached(&BTreeSet::from([schema_id(1), schema_id(2)]));

        assert_eq!(cached.keys().copied().collect::<Vec<_>>(), [schema_id(1)]);
        assert_eq!(cached[&schema_id(1)], schema("a"));
        assert_eq!(missing, BTreeSet::from([schema_id(2)]));
    }

    #[test]
    fn purge_memory_keeps_unimplemented() {
        let cache = SchemaCacheHandle::default();
        cache.insert(&BTreeMap::from([(schema_id(1), schema("a"))]));
        cache.set_get_schemas_unimplemented();

        cache.purge_memory();

        let (cached, missing) = cache.split_cached(&BTreeSet::from([schema_id(1)]));
        assert!(cached.is_empty());
        assert_eq!(missing, BTreeSet::from([schema_id(1)]));
        assert!(cache.get_schemas_unimplemented());
    }
}
