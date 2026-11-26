use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::json::writer::{JsonArray, WriterBuilder};
use serde_json::{Map, Value};

use re_log_types::EntityPath;
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::archetypes::Clear;
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

/// Component values as a JSON object keyed by entity path.
///
/// Entities are in path order, then archetypes and fields in name order, so the same values always
/// read the same, whatever order they come in.
///
/// A component whose datatype `arrow-json` cannot encode is written as `{ "$error": "…" }` in place
/// of its value, so one exotic component does not hide the rest. So is one whose value is an `Err`.
///
/// `Clear` components are left out. The caller applies them when it queries the values, as
/// `StorageEngineReadGuard::latest_at_all` does.
pub fn json_from_components<E: std::fmt::Display>(
    reflection: &Reflection,
    components: impl IntoIterator<Item = (EntityPath, ComponentDescriptor, Result<ArrayRef, E>)>,
) -> Value {
    re_tracing::profile_function!();

    let clear_component = Clear::descriptor_is_recursive().component;

    let mut components_per_entity: BTreeMap<EntityPath, Vec<_>> = BTreeMap::new();
    for (entity_path, descriptor, value) in components {
        if descriptor.component != clear_component {
            components_per_entity
                .entry(entity_path)
                .or_default()
                .push((descriptor, value));
        }
    }

    let mut entities = Map::new();
    for (entity_path, mut components) in components_per_entity {
        components.sort_by_key(|(descriptor, _)| descriptor.component.as_str());
        let mut archetypes = BTreeMap::<&str, Map<String, Value>>::new();
        let mut loose_components = BTreeMap::<&str, Value>::new();
        for (descriptor, value) in &components {
            let value = match value {
                Err(err) => serde_json::json!({ "$error": err.to_string() }),
                Ok(batch) => {
                    if batch.is_empty() || batch.data_type() == &DataType::Null {
                        continue;
                    }
                    json_from_component_batch(descriptor, batch.clone(), &reflection.components)
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
    use std::sync::Arc;

    use serde_json::json;

    use re_log_types::{TimeCell, TimePoint, TimelineName};

    use crate::partial_chunks_from_json;
    use crate::test_util::{empty_blueprint, example_blueprint, read};

    /// A `Clear` hides its own entity, its descendants only if recursive, and nothing newer.
    #[test]
    fn clears_shadow_older_values() {
        let parent = "/view/aaaaaaaa-0000-4000-8000-000000000001";
        let child = format!("{parent}/child");
        let named = |name: &str| json!({"ViewBlueprint": {"display_name": name}});

        let mut blueprint = empty_blueprint();
        let mut time = 0;
        let mut read_after = |edit: serde_json::Value| {
            time += 1;
            let timeline = TimelineName::from_static_str("blueprint");
            for chunk in partial_chunks_from_json(
                &edit,
                &TimePoint::from([(timeline, TimeCell::from_sequence(time))]),
                re_sdk_types::reflection::reflection(),
            )
            .unwrap()
            {
                blueprint.add_chunk(&Arc::new(chunk)).unwrap();
            }
            read(&blueprint)
        };

        let both = json!({parent: named("parent"), &child: named("child")});
        assert_eq!(read_after(both.clone()), both);
        assert_eq!(
            read_after(json!({parent: {"Clear": {"is_recursive": false}}})),
            json!({&child: named("child")})
        );
        assert_eq!(read_after(both.clone()), both);
        assert_eq!(
            read_after(json!({parent: {"Clear": {"is_recursive": true}}})),
            json!({})
        );
        assert_eq!(
            read_after(json!({&child: named("restored")})),
            json!({&child: named("restored")})
        );
    }

    /// The ids, and the entity paths built from them, the way the viewer and the SDKs lay them out.
    #[test]
    fn blueprint_rows_become_json() {
        insta::assert_snapshot!(serde_json::to_string_pretty(&read(&example_blueprint())).unwrap());
    }
}
