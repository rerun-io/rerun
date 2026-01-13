use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, FixedSizeListArray, ListArray, RecordBatch, RecordBatchOptions,
};
use arrow::datatypes::{Field, Schema};
use arrow::error::ArrowError;
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::LatestAtQuery;
use re_log_types::{EntityPath, TimeInt, TimelineName};
use re_sorbet::ComponentColumnDescriptor;
use re_types_core::ComponentDescriptor;

use crate::{ChunkStore, OnMissingChunk, QueryResults};

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
        let mut fields = vec![];
        let mut data = vec![];

        for entity in self
            .all_entities()
            .into_iter()
            .filter(EntityPath::is_property)
        {
            let QueryResults { chunks, missing } = self
                // TODO(zehiko) we should be able to get static chunks without specifying the timeline
                .latest_at_relevant_chunks_for_all_components(
                    OnMissingChunk::Report,
                    &LatestAtQuery::new(
                        TimelineName::log_tick(), /* timeline is irrelevant, these are static chunks */
                        TimeInt::MIN,
                    ),
                    &entity,
                    true, /* yes, we want static chunks */
                );

            if !missing.is_empty() {
                return Err(ExtractPropertiesError::MissingData(missing));
            }

            for chunk in chunks {
                for component_desc in chunk.component_descriptors() {
                    let component = component_desc.component;

                    // it's possible to have multiple values for the same component, hence we take the latest value
                    let chunk_comp_latest = chunk.latest_at(
                        /* same as above, timeline is irrelevant as these are static chunks */
                        &LatestAtQuery::new(TimelineName::log_tick(), TimeInt::MIN),
                        component,
                    );
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
                    let mut list_array = ListArray::from(column.list_array.clone());

                    // we strip __properties from the entity path, see
                    // <https://www.notion.so/rerunio/Canonical-column-identifier-for-properties-215b24554b1980029ff1cc6cdfad3f76>
                    // NOTE: we need to handle both `/__properties` AND `/__properties/$FOO` here
                    let name = property_column_name(&entity, component_desc);

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
                    let mut new_field =
                        Field::new(name.clone(), list_array.data_type().clone(), nullable);

                    // TODO(rerun-io/dataplatform#567) it seems we're hitting https://github.com/lance-format/lance/issues/2304. So what happens is that
                    // we store a properties with a FixedSizeList and Lance stores it as nullable = true, regardless of the input
                    // field. If we then try to register another partition with the same property, but with nullable = false, we'll
                    // get a "Cannot change field type for field" error. Hence, we have to make field nullable in case of FixedSizeList
                    // Also see `register_one_partition_then_another_with_same_property` test.
                    // let list_array: &dyn arrow::array::Array = &list_array;
                    let list_array_values = (&list_array as &dyn arrow::array::Array)
                        .try_downcast_array_ref::<ListArray>()?
                        .values();

                    if let arrow::datatypes::DataType::FixedSizeList(
                        fixed_size_list_inner,
                        length,
                    ) = list_array_values.data_type()
                    {
                        let inner_field = Arc::new(
                            (**fixed_size_list_inner)
                                .clone()
                                .with_nullable(true /* now nullable */),
                        );

                        let fixed_size_list_field = Arc::new(Field::new(
                            "item",
                            arrow::datatypes::DataType::FixedSizeList(inner_field.clone(), *length),
                            true, /* nullable */
                        ));

                        let array =
                            list_array_values.try_downcast_array_ref::<FixedSizeListArray>()?;
                        let values = array.values();
                        let nulls = array.nulls();

                        // we have to recreate the FixedSizeListArray with the new field's nullable field definition
                        let new_fixed_size_list = FixedSizeListArray::try_new(
                            inner_field,
                            *length,
                            values.clone(),
                            nulls.cloned(),
                        )?;

                        let array = ListArray::try_new(
                            fixed_size_list_field.clone(),
                            list_array.offsets().clone(),
                            Arc::new(new_fixed_size_list) as ArrayRef,
                            list_array.nulls().cloned(),
                        )?;

                        let field_nullable = Field::new(
                            name,
                            arrow::datatypes::DataType::List(fixed_size_list_field),
                            nullable,
                        );

                        new_field = field_nullable;
                        list_array = array;
                    }

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
