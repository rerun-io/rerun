use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::json::writer::{JsonArray, WriterBuilder};
use serde_json::{Map, Value};

use re_chunk_store::{ChunkStore, ChunkTrackingMode, LatestAtQuery};
use re_log_types::EntityPath;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::reflection::{ComponentDescriptorExt as _, ComponentReflectionMap, Reflection};

use crate::archetype_key::json_key_of_archetype;
use crate::arrow_union::UnionEncoderFactory;
use crate::special::special_json_from_batch;

/// Why one component could not be encoded as JSON.
#[derive(Debug, thiserror::Error)]
enum ComponentToJsonError {
    #[error("failed to encode as JSON: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("failed to read for its special encoding: {0}")]
    Special(#[from] re_sdk_types::DeserializationError),

    #[error("arrow-json wrote invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Every entity of `store` at `query`, as a JSON object keyed by entity path.
///
/// Entities are in path order, then archetypes and fields in name order, so the same store always
/// reads the same.
///
/// A component whose datatype `arrow-json` cannot encode is written as `{ "$error": "…" }` in place
/// of its value, so one exotic component does not hide the rest. So is one whose latest value is in
/// a chunk that is not loaded yet.
pub fn json_from_store(
    store: &ChunkStore,
    query: &LatestAtQuery,
    reflection: &Reflection,
) -> Value {
    re_tracing::profile_function!();

    let mut descriptors_per_entity: BTreeMap<&EntityPath, Vec<_>> = BTreeMap::new();
    for (entity_path, entry) in store.schema().all_column_metadata() {
        descriptors_per_entity
            .entry(entity_path)
            .or_default()
            .push(&entry.descriptor);
    }
    for descriptors in descriptors_per_entity.values_mut() {
        descriptors.sort_by_key(|descriptor| descriptor.component.as_str());
    }

    let mut entities = Map::new();
    for (entity_path, descriptors) in descriptors_per_entity {
        let mut archetypes = BTreeMap::<&str, Map<String, Value>>::new();
        let mut loose_components = BTreeMap::<&str, Value>::new();
        for descriptor in descriptors {
            let value = match latest_at(store, query, entity_path, descriptor.component) {
                LatestAt::Missing => continue,
                LatestAt::NotLoaded => {
                    serde_json::json!({ "$error": "its latest value is not loaded yet" })
                }
                LatestAt::Batch(batch) => {
                    if batch.is_empty() || batch.data_type() == &DataType::Null {
                        continue;
                    }
                    json_from_component_batch(descriptor, batch, &reflection.components)
                        .unwrap_or_else(|err| serde_json::json!({ "$error": err.to_string() }))
                }
            };

            if let Some(archetype) = descriptor.archetype {
                archetypes
                    .entry(json_key_of_archetype(reflection, archetype))
                    .or_default()
                    .insert(descriptor.archetype_field_name().to_owned(), value);
            } else {
                loose_components.insert(descriptor.component.as_str(), value);
            }
        }

        let mut entity: Map<String, Value> = archetypes
            .into_iter()
            .map(|(archetype, fields)| (archetype.to_owned(), Value::Object(fields)))
            .collect();
        for (component, value) in loose_components {
            // Both are keyed by name, so an archetype wins over a loose component called the same.
            if entity.contains_key(component) {
                re_log::warn_once!(
                    "Leaving component {component:?} out of the blueprint JSON of {entity_path}, since an archetype there has the same name"
                );
            } else {
                entity.insert(component.to_owned(), value);
            }
        }
        if !entity.is_empty() {
            entities.insert(entity_path.to_string(), Value::Object(entity));
        }
    }

    Value::Object(entities)
}

/// What [`latest_at`] found.
enum LatestAt {
    /// Nothing was logged for the component at or before the query time.
    Missing,

    /// The latest value is in a chunk the store only knows of, and has not loaded.
    NotLoaded,

    Batch(ArrayRef),
}

/// The latest batch of `component` on `entity_path` at `query`.
fn latest_at(
    store: &ChunkStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    component: ComponentIdentifier,
) -> LatestAt {
    let results =
        store.latest_at_relevant_chunks(ChunkTrackingMode::Report, query, entity_path, component);
    if results.is_partial() {
        return LatestAt::NotLoaded;
    }
    results
        .chunks
        .iter()
        .filter_map(|chunk| {
            let unit = chunk.latest_at(query, component)?;
            let index = unit.index(query.timeline().as_ref())?;
            Some((index, unit))
        })
        .max_by_key(|(index, _)| *index)
        .and_then(|(_, unit)| unit.component_batch_raw(component))
        .map_or(LatestAt::Missing, LatestAt::Batch)
}

/// One component batch as JSON: the instance itself for a batch of one, else an array.
fn json_from_component_batch(
    descriptor: &re_sdk_types::ComponentDescriptor,
    batch: ArrayRef,
    components: &ComponentReflectionMap,
) -> Result<Value, ComponentToJsonError> {
    let instances = match special_json_from_batch(descriptor, &batch, components) {
        Some(special) => special?,
        None => json_from_arrow(batch)?,
    };

    Ok(match <[Value; 1]>::try_from(instances) {
        Ok([instance]) => instance,
        Err(instances) => Value::Array(instances),
    })
}

/// Each instance of `batch`, encoded by `arrow-json`.
fn json_from_arrow(batch: ArrayRef) -> Result<Vec<Value>, ComponentToJsonError> {
    const COLUMN: &str = "value";

    let schema = Schema::new_with_metadata(
        vec![Field::new(COLUMN, batch.data_type().clone(), true)],
        Default::default(),
    );
    let options = RecordBatchOptions::new().with_row_count(Some(batch.len()));
    let record_batch = RecordBatch::try_new_with_options(Arc::new(schema), vec![batch], &options)?;

    let mut writer = WriterBuilder::new()
        .with_explicit_nulls(true)
        .with_encoder_factory(Arc::new(UnionEncoderFactory))
        .build::<_, JsonArray>(Vec::new());
    writer.write(&record_batch)?;
    writer.finish()?;

    let rows: Vec<Map<String, Value>> = serde_json::from_slice(&writer.into_inner())?;
    Ok(rows
        .into_iter()
        .map(|mut row| row.remove(COLUMN).unwrap_or(Value::Null))
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::test_util::{example_blueprint, read};

    /// The ids, and the entity paths built from them, the way the viewer and the SDKs lay them out.
    #[test]
    fn blueprint_rows_become_json() {
        insta::assert_snapshot!(serde_json::to_string_pretty(&read(&example_blueprint())).unwrap());
    }
}
