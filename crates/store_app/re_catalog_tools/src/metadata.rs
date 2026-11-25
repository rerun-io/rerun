//! The read-only tools that describe a catalog: entries, datasets, schemas, segments, and
//! properties.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use re_catalog_client::{CatalogClient, DatasetInfo};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::EntryDetails;
use re_protos::cloud::v1alpha1::{DatasetQueryScope, EntryFilter, EntryKind};
use re_redap_client::ApiErrorKind;
use re_types_core::SegmentId;

use crate::CatalogToolError;
use crate::args::{
    DatasetArgs, DatasetSchemaArgs, EntryKindArg, ListEntriesArgs, ListSegmentsArgs,
    SegmentPropertiesArgs,
};

/// The id of the dataset `dataset` names, either by id or by name.
///
/// An id is looked up too, so that an unknown id, or the id of a table, gets the same hint as an
/// unknown name, and a dataset whose name looks like an id can still be named.
pub async fn dataset_id(
    catalog: &CatalogClient,
    dataset: &str,
) -> Result<EntryId, CatalogToolError> {
    find_entry(catalog, dataset, &[EntryKind::Dataset])
        .await?
        .map(|entry| entry.id)
        .ok_or_else(|| CatalogToolError::NoSuchDataset {
            dataset: dataset.to_owned(),
        })
}

/// The entry of one of `kinds` that `entry` names, either by id or by name, if any.
///
/// Empty `kinds` means any kind.
pub async fn find_entry(
    catalog: &CatalogClient,
    entry: &str,
    kinds: &[EntryKind],
) -> Result<Option<EntryDetails>, CatalogToolError> {
    let filter = EntryFilter::new().with_entry_kinds(kinds.iter().copied());
    if let Ok(id) = entry.parse::<EntryId>()
        && let Some(details) = find_one(catalog, filter.clone().with_id(id)).await?
    {
        return Ok(Some(details));
    }
    find_one(catalog, filter.with_name(entry)).await
}

/// The first entry matching `filter`, if any.
async fn find_one(
    catalog: &CatalogClient,
    filter: EntryFilter,
) -> Result<Option<EntryDetails>, CatalogToolError> {
    match catalog.find_entries(filter).await {
        Ok(entries) => Ok(entries.into_iter().next()),
        Err(err) if err.kind == ApiErrorKind::NotFound => Ok(None),
        Err(err) => Err(CatalogToolError::catalog("look up the entry", err)),
    }
}

pub async fn list_entries(
    catalog: &CatalogClient,
    args: ListEntriesArgs,
) -> Result<Value, CatalogToolError> {
    let ListEntriesArgs {
        kind,
        offset,
        limit,
    } = args;
    let kinds = match kind {
        Some(EntryKindArg::Dataset) => vec![EntryKind::Dataset],
        Some(EntryKindArg::Table) => vec![EntryKind::Table],
        None => vec![EntryKind::Dataset, EntryKind::Table],
    };
    let mut entries = catalog
        .find_entries(EntryFilter::new().with_entry_kinds(kinds))
        .await
        .map_err(|err| CatalogToolError::catalog("list the entries", err))?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let page: Vec<Value> = page(&entries, offset, limit, ListEntriesArgs::MAX_ENTRY_PAGE)
        .iter()
        .map(|entry| {
            let EntryDetails {
                id,
                name,
                kind,
                created_at: _,
                updated_at,
            } = entry;
            json!({
                "id": id.to_string(),
                "name": name.to_string(),
                "kind": entry_kind_name(*kind),
                "updated_at": updated_at.to_string(),
            })
        })
        .collect();
    Ok(json!({
        "catalog": catalog.origin().to_string(),
        "num_entries": entries.len(),
        "offset": offset,
        "entries": page,
    }))
}

fn entry_kind_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Dataset => "dataset",
        EntryKind::Table => "table",
        EntryKind::DatasetView => "dataset_view",
        EntryKind::TableView => "table_view",
        EntryKind::BlueprintDataset => "blueprint_dataset",
        EntryKind::AssetDataset => "asset_dataset",
        EntryKind::Unspecified => "unspecified",
    }
}

pub async fn get_dataset_info(
    catalog: &CatalogClient,
    args: DatasetArgs,
) -> Result<Value, CatalogToolError> {
    let DatasetArgs { dataset } = args;
    let id = dataset_id(catalog, &dataset).await?;
    let DatasetInfo {
        num_segments,
        size_bytes,
        layers,
        timelines,
        num_entities,
        num_component_columns,
        property_columns,
    } = catalog
        .get_dataset_info(id)
        .await
        .map_err(|err| CatalogToolError::catalog("read the dataset info", err))?;
    Ok(json!({
        "id": id.to_string(),
        "num_segments": num_segments,
        "size_bytes": size_bytes,
        "layers": layers.iter().map(|layer| json!({
            "name": layer.name.to_string(),
            "num_segments": layer.num_segments,
        })).collect::<Vec<_>>(),
        "timelines": timelines.iter().map(|timeline| json!({
            "name": timeline.name.to_string(),
            "num_segments": timeline.num_segments,
            "range": timeline.typ.format_range_utc(timeline.range),
        })).collect::<Vec<_>>(),
        "num_entities": num_entities,
        "num_component_columns": num_component_columns,
        "property_columns": property_columns
            .iter()
            .map(|column| column.display_name())
            .collect::<Vec<_>>(),
    }))
}

pub async fn get_dataset_schema(
    catalog: &CatalogClient,
    args: DatasetSchemaArgs,
) -> Result<Value, CatalogToolError> {
    let DatasetSchemaArgs {
        dataset,
        entity_path,
        offset,
    } = args;
    let id = dataset_id(catalog, &dataset).await?;
    let schema = catalog
        .get_dataset_schema(id, DatasetQueryScope::Full)
        .await
        .map_err(|err| CatalogToolError::catalog("read the dataset schema", err))?
        .schema;
    let columns = re_sorbet::SorbetColumnDescriptors::try_from_arrow_fields(None, schema.fields())
        .map_err(|source| CatalogToolError::Schema { source })?;

    let timelines: Vec<Value> = columns
        .index_columns()
        .map(|column| {
            json!({
                "name": column.timeline_name().to_string(),
                "datatype": column.datatype().to_string(),
            })
        })
        .collect();

    if let Some(entity_path) = entity_path {
        let entity_path = re_log_types::EntityPath::parse_forgiving(&entity_path);
        let components: Vec<Value> = columns
            .component_columns()
            .filter(|column| column.entity_path == entity_path)
            .map(|column| {
                json!({
                    "archetype": column.archetype.as_ref().map(|a| a.short_name()),
                    "component": column.component.to_string(),
                    "component_type": column.component_type.as_ref().map(|c| c.to_string()),
                    "datatype": column.store_datatype.to_string(),
                    "is_static": column.is_static,
                })
            })
            .collect();
        if components.is_empty() {
            return Err(CatalogToolError::NoSuchEntity { entity_path });
        }
        return Ok(json!({
            "timelines": timelines,
            "entity_path": entity_path.to_string(),
            "components": components,
        }));
    }

    let mut entities: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for column in columns.component_columns() {
        let archetypes = entities.entry(column.entity_path.to_string()).or_default();
        if let Some(archetype) = &column.archetype {
            archetypes.insert(archetype.short_name());
        }
    }
    let entities: Vec<(String, BTreeSet<&str>)> = entities.into_iter().collect();
    let max = DatasetSchemaArgs::MAX_SCHEMA_ENTITIES;
    let entity_page: BTreeMap<&String, &BTreeSet<&str>> = page(&entities, offset, max, max)
        .iter()
        .map(|(entity, archetypes)| (entity, archetypes))
        .collect();
    Ok(json!({
        "timelines": timelines,
        "num_entities": entities.len(),
        "offset": offset,
        "entities": entity_page,
    }))
}

/// Each page reads and sorts the dataset's whole list of segment ids, since the catalog cannot
/// page through it, so paging through a large dataset costs more than its pages suggest.
pub async fn list_segments(
    catalog: &CatalogClient,
    args: ListSegmentsArgs,
) -> Result<Value, CatalogToolError> {
    let ListSegmentsArgs {
        dataset,
        offset,
        limit,
    } = args;
    let id = dataset_id(catalog, &dataset).await?;
    let segment_ids = catalog
        .get_dataset_segment_ids(id)
        .await
        .map_err(|err| CatalogToolError::catalog("list the segments", err))?;
    Ok(segment_page(segment_ids, offset, limit))
}

/// One page of `segment_ids`, sorted, since the server's order may change between pages.
fn segment_page(mut segment_ids: Vec<SegmentId>, offset: usize, limit: usize) -> Value {
    segment_ids.sort();
    let page: Vec<&str> = page(
        &segment_ids,
        offset,
        limit,
        ListSegmentsArgs::MAX_SEGMENT_PAGE,
    )
    .iter()
    .map(|segment_id| segment_id.as_str())
    .collect();
    json!({
        "num_segments": segment_ids.len(),
        "offset": offset,
        "segment_ids": page,
    })
}

/// The `limit` items of `items` after the first `offset`, but never more than `max`.
fn page<T>(items: &[T], offset: usize, limit: usize, max: usize) -> &[T] {
    let start = offset.min(items.len());
    let end = start.saturating_add(limit.min(max)).min(items.len());
    &items[start..end]
}

pub async fn get_segment_properties(
    catalog: &CatalogClient,
    args: SegmentPropertiesArgs,
) -> Result<Value, CatalogToolError> {
    let SegmentPropertiesArgs {
        dataset,
        segment_id: segment,
    } = args;
    let id = dataset_id(catalog, &dataset).await?;
    let segment_id = SegmentId::new(segment.clone());
    let properties = catalog
        .get_segment_properties(id, segment_id.clone())
        .await
        .map_err(|err| CatalogToolError::catalog("read the segment properties", err))?;
    let properties = if let Some((properties, _revision)) = properties {
        first_row_json(&properties)?
    } else {
        match catalog.ensure_segment_exists(id, segment_id).await {
            Ok(()) => json!({}),
            Err(err) if err.kind == ApiErrorKind::NotFound => {
                return Err(CatalogToolError::NoSuchSegment {
                    segment_id: segment,
                });
            }
            Err(err) => return Err(CatalogToolError::catalog("look up the segment", err)),
        }
    };
    Ok(json!({
        "segment_id": segment,
        "properties": properties,
    }))
}

/// The first row of `batch` as a JSON object keyed by column name.
fn first_row_json(batch: &arrow::array::RecordBatch) -> Result<Value, CatalogToolError> {
    let mut writer = arrow::json::ArrayWriter::new(Vec::new());
    writer
        .write(batch)
        .and_then(|()| writer.finish())
        .map_err(|source| CatalogToolError::EncodeJson { source })?;
    let rows: Vec<Value> = serde_json::from_slice(&writer.into_inner())
        .map_err(|source| CatalogToolError::DecodeJson { source })?;
    Ok(rows.into_iter().next().unwrap_or_else(|| json!({})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_capped_and_never_out_of_bounds() {
        let items: Vec<usize> = (0..10).collect();
        assert_eq!(page(&items, 2, 3, 100), [2, 3, 4]);
        assert_eq!(page(&items, 8, 3, 100), [8, 9]);
        assert_eq!(page(&items, 20, 3, 100), [0_usize; 0]);
        assert_eq!(page(&items, 0, usize::MAX, 4), [0, 1, 2, 3]);
    }

    #[test]
    fn segment_pages_are_sorted_and_capped() {
        let segment_ids: Vec<SegmentId> = (0..2000)
            .rev()
            .map(|i| SegmentId::new(format!("segment_{i:04}")))
            .collect();
        let first = segment_page(segment_ids.clone(), 0, 3);
        assert_eq!(
            first,
            json!({
                "num_segments": 2000,
                "offset": 0,
                "segment_ids": ["segment_0000", "segment_0001", "segment_0002"],
            })
        );
        let capped = segment_page(segment_ids, 0, usize::MAX);
        assert_eq!(
            capped["segment_ids"].as_array().map(Vec::len),
            Some(ListSegmentsArgs::MAX_SEGMENT_PAGE)
        );
    }
}
