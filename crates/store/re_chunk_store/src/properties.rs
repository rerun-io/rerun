use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, FixedSizeListArray, ListArray, RecordBatch, RecordBatchOptions,
};
use arrow::datatypes::{Field, Schema};
use arrow::error::ArrowError;
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, ChunkId, LatestAtQuery};
use re_log_types::{EntityPath, TimeInt, TimelineName};
use re_sorbet::ComponentColumnDescriptor;
use re_types_core::ComponentDescriptor;

use crate::{ChunkStore, ChunkTrackingMode, QueryResults};

#[derive(Debug, thiserror::Error)]
pub enum ExtractPropertiesError {
    #[error("{0}")]
    ArrowError(#[from] ArrowError),

    #[error("{0}")]
    Internal(String),

    #[error(
        "partially loaded store: properties cannot reliably be computed, fetch the missing chunks first"
    )]
    MissingData(Vec<re_chunk::ChunkId>),
}

impl ChunkStore {
    /// Extract a one-row [`RecordBatch`] containing the properties for this chunk store.
    ///
    /// The column names are based on the following proposals and are further sanitized to ensure
    /// compatibility with Lance datasets.
    ///
    /// <https://www.notion.so/rerunio/Canonical-column-identifier-for-dataframe-queries-206b24554b1980d98454eb989703ce2b>
    /// <https://www.notion.so/rerunio/Canonical-column-identifier-for-properties-215b24554b1980029ff1cc6cdfad3f76>
    // TODO(ab): move these ^ to a better place.
    pub fn extract_properties(&self) -> Result<RecordBatch, ExtractPropertiesError> {
        let per_entity = self.property_entities_query_results();

        let all_missing: Vec<_> = per_entity
            .iter()
            .flat_map(|(_, qr)| qr.missing_virtual.iter().copied())
            .collect();
        if !all_missing.is_empty() {
            return Err(ExtractPropertiesError::MissingData(all_missing));
        }

        let per_entity_chunks: Vec<(EntityPath, Vec<Arc<Chunk>>)> = per_entity
            .into_iter()
            .map(|(entity, qr)| (entity, qr.chunks))
            .collect();

        build_properties_record_batch(&per_entity_chunks)
    }

    /// Run the property-entity latest-at queries used by both the pure-`ChunkStore` path and the
    /// split `property_entities_query_results` / [`extract_properties_from_chunks`] path used by
    /// lazy stores.
    pub fn property_entities_query_results(&self) -> Vec<(EntityPath, QueryResults)> {
        // Sweep all property entities first and collect the union of missing virtual chunks
        // across all of them. This way callers that auto-load (e.g. `LazyRrdStore::extract_properties`)
        // see the full batch in one shot and converge in a single retry instead of one disk
        // round-trip per entity.
        self.all_entities()
            .into_iter()
            .filter(EntityPath::is_property)
            .map(|entity| {
                let results = self
                    // TODO(zehiko) we should be able to get static chunks without specifying the timeline
                    .latest_at_relevant_chunks_for_all_components(
                        ChunkTrackingMode::Report,
                        &LatestAtQuery::new(
                            TimelineName::log_tick(), /* timeline is irrelevant, these are static chunks */
                            TimeInt::MIN,
                        ),
                        &entity,
                        true, /* yes, we want static chunks */
                    );
                (entity, results)
            })
            .collect()
    }
}

/// Build a one-row properties [`RecordBatch`] from a pre-materialized slice of chunks plus the
/// per-entity query results that produced the chunk-id list.
///
/// `chunks` must contain every chunk referenced by `per_entity_results` (both the already-resolved
/// `chunks` and the `missing_virtual` ids). Returns [`ExtractPropertiesError::MissingData`] listing
/// any ids that aren't present in `chunks`.
pub fn extract_properties_from_chunks(
    per_entity_results: &[(EntityPath, QueryResults)],
    chunks: &[Arc<Chunk>],
) -> Result<RecordBatch, ExtractPropertiesError> {
    use ahash::HashMap;

    let chunks_by_id: HashMap<ChunkId, &Arc<Chunk>> = chunks.iter().map(|c| (c.id(), c)).collect();

    let mut missing: Vec<ChunkId> = Vec::new();
    let per_entity_chunks: Vec<(EntityPath, Vec<Arc<Chunk>>)> = per_entity_results
        .iter()
        .map(|(entity, qr)| {
            let materialized: Vec<Arc<Chunk>> = qr
                .chunks
                .iter()
                .map(|c| c.id())
                .chain(qr.missing_virtual.iter().copied())
                .filter_map(|id| {
                    if let Some(c) = chunks_by_id.get(&id) {
                        Some(Arc::clone(c))
                    } else {
                        missing.push(id);
                        None
                    }
                })
                .collect();
            (entity.clone(), materialized)
        })
        .collect();

    if !missing.is_empty() {
        return Err(ExtractPropertiesError::MissingData(missing));
    }

    build_properties_record_batch(&per_entity_chunks)
}

fn build_properties_record_batch(
    per_entity_chunks: &[(EntityPath, Vec<Arc<Chunk>>)],
) -> Result<RecordBatch, ExtractPropertiesError> {
    let mut fields = vec![];
    let mut data = vec![];

    for (entity, chunks) in per_entity_chunks {
        for chunk in chunks {
            for component_desc in chunk.component_descriptors() {
                let component = component_desc.component;

                // it's possible to have multiple values for the same component, hence we take the latest value
                let Some(chunk_comp_latest) = chunk.latest_at(
                    /* same as above, timeline is irrelevant as these are static chunks */
                    &LatestAtQuery::new(TimelineName::log_tick(), TimeInt::MIN),
                    component,
                ) else {
                    continue;
                };
                let (_, column) = chunk_comp_latest
                    .components()
                    .iter()
                    .find(|(c, _)| **c == component)
                    .ok_or({
                        // this should never happen really
                        ExtractPropertiesError::Internal(format!(
                            "failed to find component in chunk: {component:?}"
                        ))
                    })?;

                let store_datatype = column.list_array.data_type().clone();
                let list_array = ListArray::from(column.list_array.clone());

                // we strip __properties from the entity path, see
                // <https://www.notion.so/rerunio/Canonical-column-identifier-for-properties-215b24554b1980029ff1cc6cdfad3f76>
                // NOTE: we need to handle both `/__properties` AND `/__properties/$FOO` here
                let name = property_column_name(entity, component_desc);

                let column_descriptor = ComponentColumnDescriptor {
                    component_type: component_desc.component_type,
                    entity_path: entity.clone(),
                    archetype: component_desc.archetype,
                    component: component_desc.component,
                    store_datatype,
                    is_semantically_empty: false,
                    is_static: false,
                    is_tombstone: false,
                };

                let metadata = column_descriptor
                    .to_arrow_field(re_sorbet::BatchType::Dataframe)
                    .metadata()
                    .clone();

                let nullable = true; // we can have partitions that don't have properties like other partitions

                let (new_field, list_array) =
                    relax_fixed_size_list_nullability(list_array, &name, nullable)?;

                let new_field = new_field.with_metadata(metadata);

                fields.push(new_field);
                data.push(re_arrow_util::into_arrow_ref(list_array));
            }
        }
    }

    let (fields, data): (Vec<_>, Vec<_>) = fields
        .into_iter()
        .zip(data)
        .sorted_by(|(field1, _), (field2, _)| field1.name().cmp(field2.name()))
        .unzip();

    Ok(RecordBatch::try_new_with_options(
        Arc::new(Schema::new_with_metadata(fields, Default::default())),
        data,
        &RecordBatchOptions::default().with_row_count(Some(1)),
    )?)
}

/// Make the inner `FixedSizeList` field nullable, working around
/// [lance-format/lance#2304](https://github.com/lance-format/lance/issues/2304).
///
/// Lance stores `FixedSizeList` properties as `nullable = true` regardless of the input field, so
/// re-registering the same property as `nullable = false` later fails with "Cannot change field
/// type for field". We force-relax it on our side to avoid that. See also
/// `register_one_partition_then_another_with_same_property` and `rerun-io/dataplatform#567`.
///
/// Returns `(field, list_array)` unchanged when the inner type isn't a `FixedSizeList`.
//TODO(RR-2041): clean this when the upstream issue is resolved
fn relax_fixed_size_list_nullability(
    list_array: ListArray,
    name: &str,
    outer_nullable: bool,
) -> Result<(Field, ListArray), ExtractPropertiesError> {
    let field = Field::new(name, list_array.data_type().clone(), outer_nullable);

    let arrow::datatypes::DataType::FixedSizeList(fixed_size_list_inner, length) =
        list_array.values().data_type()
    else {
        return Ok((field, list_array));
    };

    let inner_field = Arc::new((**fixed_size_list_inner).clone().with_nullable(true));
    let fixed_size_list_field = Arc::new(Field::new(
        "item",
        arrow::datatypes::DataType::FixedSizeList(inner_field.clone(), *length),
        true, /* nullable */
    ));

    let inner_fixed = list_array
        .values()
        .try_downcast_array_ref::<FixedSizeListArray>()?;
    let new_fixed_size_list = FixedSizeListArray::try_new(
        inner_field,
        *length,
        inner_fixed.values().clone(),
        inner_fixed.nulls().cloned(),
    )?;

    let new_list_array = ListArray::try_new(
        fixed_size_list_field.clone(),
        list_array.offsets().clone(),
        Arc::new(new_fixed_size_list) as ArrayRef,
        list_array.nulls().cloned(),
    )?;

    let new_field = Field::new(
        name,
        arrow::datatypes::DataType::List(fixed_size_list_field),
        outer_nullable,
    );

    Ok((new_field, new_list_array))
}

fn property_column_name(entity_path: &EntityPath, component_desc: &ComponentDescriptor) -> String {
    use re_types_core::reflection::ComponentDescriptorExt as _;
    [
        Some("property".to_owned()),
        Some({
            let path = entity_path
                .strip_prefix(&EntityPath::properties())
                .unwrap_or_else(|| entity_path.clone())
                .to_string();
            let path = path.strip_prefix("/").unwrap_or(&path);
            path.strip_suffix("/").unwrap_or(path).to_owned()
        }),
        component_desc
            .archetype
            .map(|archetype| archetype.short_name().to_owned()),
        Some(component_desc.archetype_field_name().to_owned()),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(":")
    // Lance doesn't allow some of the special characters in the column names.
    // This function replaces those special characters with `_`.
    .replace([',', ' ', '-', '.', '\\'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_column_name() {
        let entity: EntityPath = "/my_entity".into();
        let component_desc = ComponentDescriptor::partial("my_component");

        let name = property_column_name(&entity, &component_desc);
        assert_eq!(name, "property:my_entity:my_component");

        let entity: EntityPath = "/a/b/c/".into();
        let component_desc_full = ComponentDescriptor::partial("field_name")
            .with_component_type("my_component_type".into())
            .with_archetype("archetype_name".into());
        let name = property_column_name(&entity, &component_desc_full);

        assert_eq!(name, "property:a/b/c:archetype_name:field_name");

        let entity: EntityPath = "/__properties/a/b/c/".into();
        let component_desc_full = ComponentDescriptor::partial("field_name")
            .with_component_type("my_component_type".into())
            .with_archetype("archetype_name".into());
        let name = property_column_name(&entity, &component_desc_full);

        assert_eq!(name, "property:a/b/c:archetype_name:field_name");
    }
}
