//! Narrowing what a scan asks the server for, driven by the DataFusion
//! projection and filters: which entity paths and components the query
//! actually touches, and the output schema that follows from a
//! [`QueryExpression`].

use arrow::datatypes::{Field, Schema, SchemaRef};
use datafusion::common::{DataFusionError, exec_datafusion_err};
use datafusion::logical_expr::Expr;
use re_dataframe::QueryExpression;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_log_types::EntityPath;
use re_sorbet::{BatchType, ChunkColumnDescriptors, ColumnDescriptor, ColumnKind};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Extract entity paths referenced by the projected columns and filter expressions.
///
/// Returns `None` when no narrowing is possible (`projection` is `None`).
/// Returns `Some(empty set)` when projection contains only non-entity columns
/// (e.g. time / `segment_id`) — caller should not narrow in this case.
pub(super) fn extract_projected_entity_paths(
    schema: &SchemaRef,
    projection: &Vec<usize>,
    filters: &[Expr],
) -> BTreeSet<EntityPath> {
    let mut entity_paths = BTreeSet::new();

    // Collect entity paths from projected columns.
    for &idx in projection {
        if let Some(path) = entity_path_from_field(schema.field(idx)) {
            entity_paths.insert(path);
        }
    }

    // Collect entity paths from filter-referenced columns. Filters may reference
    // columns that aren't in the projection (e.g. `WHERE t.b > 5` with only `t.a`
    // projected) — we must still fetch data for those entities.
    for filter in filters {
        for col_ref in filter.column_refs() {
            if let Ok(field) = schema.field_with_name(col_ref.name())
                && let Some(path) = entity_path_from_field(field)
            {
                entity_paths.insert(path);
            }
        }
    }

    entity_paths
}

/// Extract an [`EntityPath`] from an Arrow field's metadata, if present.
///
/// Component columns carry `rerun:entity_path` metadata; time/index columns
/// and the prepended `rerun_segment_id` column do not.
fn entity_path_from_field(field: &Field) -> Option<EntityPath> {
    field
        .metadata()
        .get(re_sorbet::metadata::SORBET_ENTITY_PATH)
        .map(|s| EntityPath::from(&**s))
}

/// The component identifier of a field, or `None` if it isn't a component column.
///
/// Only genuine component columns carry an entity path; gating on it (the
/// same signal `extract_projected_entity_paths` uses) drops the prepended,
/// unmarked `rerun_segment_id`, which would otherwise be misclassified as a
/// `Component` (`ColumnKind` defaults to `Component` for unmarked fields).
/// Index/time columns also lack an entity path, so the same gate excludes
/// them (and as a backstop they carry `rerun:kind=index`, which
/// `try_from_arrow_field` maps to a non-`Component` descriptor).
fn component_from_field(field: &Field) -> Option<String> {
    field
        .metadata()
        .get(re_sorbet::metadata::SORBET_ENTITY_PATH)?;
    match ColumnDescriptor::try_from_arrow_field(None, field) {
        Ok(ColumnDescriptor::Component(component)) => Some(component.component.to_string()),
        _ => None,
    }
}

/// Every component identifier present in `schema`.
///
/// Used to detect a full projection: when the projection references every
/// component, narrowing `fuzzy_descriptors` is a no-op for chunk skipping, but
/// the server treats a non-empty list as exhaustive and would drop chunks for
/// static-only components (those with no temporal index). So we only narrow when
/// the projection is a strict subset.
pub(super) fn all_schema_components(schema: &SchemaRef) -> BTreeSet<String> {
    schema
        .fields()
        .iter()
        .filter_map(|field| component_from_field(field))
        .collect()
}

/// Component identifiers referenced by a query's projection and filters.
///
/// Counterpart to [`extract_projected_entity_paths`] at component granularity:
/// it lets the scan narrow `fuzzy_descriptors` so the server skips chunks for
/// unselected components (e.g. a heavy `VideoStream:sample` sitting next to a tiny `is_keyframe`).
/// Time/index and `rerun_segment_id` columns are not components and are simply ignored.
pub(super) fn extract_projected_components(
    schema: &SchemaRef,
    projection: &[usize],
    filters: &[Expr],
) -> BTreeSet<String> {
    let mut components = BTreeSet::new();

    for &idx in projection {
        if let Some(component) = component_from_field(schema.field(idx)) {
            components.insert(component);
        }
    }

    // Filters may reference components outside the projection (e.g. `WHERE
    // is_keyframe IS NOT NULL` while only the index is projected); those chunks
    // are still needed to evaluate the filter, so keep them too.
    for filter in filters {
        for col_ref in filter.column_refs() {
            if let Ok(field) = schema.field_with_name(col_ref.name())
                && let Some(component) = component_from_field(field)
            {
                components.insert(component);
            }
        }
    }

    components
}

/// Compute the output schema for a query on a dataset. When we call `get_dataset_schema`
/// on the catalog server, we will get the schema for all entities and all components. This
/// method is used to down select from that full schema based on `query_expression`.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn compute_schema_for_query(
    dataset_schema: &Schema,
    query_expression: &QueryExpression,
) -> Result<SchemaRef, DataFusionError> {
    // Short circuit for empty datasets. Needed because `ChunkColumnDescriptors::try_from_arrow_fields`
    // needs row ids, which we only have for non-empty datasets.
    if dataset_schema.fields.is_empty() {
        return Ok(Arc::new(Schema::empty()));
    }

    // Schema returned from `get_dataset_schema` does not match the required ChunkColumnDescriptors ordering
    // which is row id, then time, then data. We don't need perfect ordering other than that.
    let mut fields = dataset_schema
        .fields()
        .iter()
        .map(Arc::clone)
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| {
        let Ok(a) = ColumnKind::try_from(a.as_ref()) else {
            return Ordering::Equal;
        };
        let Ok(b) = ColumnKind::try_from(b.as_ref()) else {
            return Ordering::Equal;
        };

        match (a, b) {
            (ColumnKind::RowId, _) => Ordering::Less,
            (_, ColumnKind::RowId) => Ordering::Greater,
            (ColumnKind::Index, _) => Ordering::Less,
            (_, ColumnKind::Index) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });
    let fields: arrow::datatypes::Fields = fields.into();

    let column_descriptors = ChunkColumnDescriptors::try_from_arrow_fields(None, &fields)
        .map_err(|err| exec_datafusion_err!("col desc {err}"))?;

    // Create the actual filter to apply to the column descriptors
    let filter = ChunkStore::create_component_filter_from_query(query_expression);

    // When we call QueryDataset we will not return row_id, so we only select indices and
    // components from the column descriptors.
    let filtered_fields = column_descriptors
        .filter_components(filter)
        .indices_and_components()
        .into_iter()
        .map(|cd| cd.to_arrow_field(BatchType::Dataframe))
        .collect::<Vec<_>>();

    Ok(Arc::new(Schema::new_with_metadata(
        filtered_fields,
        dataset_schema.metadata().clone(),
    )))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, iter::once};

    use arrow::datatypes::DataType;
    use re_dataframe::SparseFillStrategy;
    use re_protos::cloud::v1alpha1::ext::QueryDatasetRequest;

    use super::*;

    /// Build a schema mimicking `DataframeQueryTableProvider`'s output schema:
    /// - Index 0: `rerun_segment_id` (Utf8, no entity path metadata)
    /// - Index 1: `log_time` (Int64, with `rerun:kind=index` metadata)
    /// - Index 2: `/points:Position3D:positions` (component, `entity_path=/points`)
    /// - Index 3: `/points:Color:colors` (component, `entity_path=/points`)
    /// - Index 4: `/cameras:Transform3D:transform` (component, `entity_path=/cameras`)
    fn make_schema_with_entities() -> SchemaRef {
        use re_sorbet::metadata::{RERUN_KIND, SORBET_ENTITY_PATH};

        let index_metadata = HashMap::from([(RERUN_KIND.to_owned(), "index".to_owned())]);
        let points_metadata =
            HashMap::from([(SORBET_ENTITY_PATH.to_owned(), "/points".to_owned())]);
        let cameras_metadata =
            HashMap::from([(SORBET_ENTITY_PATH.to_owned(), "/cameras".to_owned())]);

        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("rerun_segment_id", DataType::Utf8, false),
                Field::new("log_time", DataType::Int64, false).with_metadata(index_metadata),
                Field::new("/points:Position3D:positions", DataType::Utf8, true)
                    .with_metadata(points_metadata.clone()),
                Field::new("/points:Color:colors", DataType::Utf8, true)
                    .with_metadata(points_metadata),
                Field::new("/cameras:Transform3D:transform", DataType::Utf8, true)
                    .with_metadata(cameras_metadata),
            ],
            HashMap::new(),
        ))
    }

    /// Like [`make_schema_with_entities`] but with `rerun:component` metadata, so
    /// component columns parse to their real identifiers (`positions`, `colors`,
    /// `transform`) instead of falling back to the full column name — matching
    /// the metadata that real dataset schemas carry.
    fn make_schema_with_components() -> SchemaRef {
        use re_sorbet::metadata::{RERUN_KIND, SORBET_ENTITY_PATH};
        // `re_types_core::FIELD_METADATA_KEY_COMPONENT`.
        const COMPONENT: &str = "rerun:component";

        let index_metadata = HashMap::from([(RERUN_KIND.to_owned(), "index".to_owned())]);
        let component_metadata = |entity: &str, component: &str| {
            HashMap::from([
                (SORBET_ENTITY_PATH.to_owned(), entity.to_owned()),
                (COMPONENT.to_owned(), component.to_owned()),
            ])
        };

        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("rerun_segment_id", DataType::Utf8, false),
                Field::new("log_time", DataType::Int64, false).with_metadata(index_metadata),
                Field::new("/points:Position3D:positions", DataType::Utf8, true)
                    .with_metadata(component_metadata("/points", "positions")),
                Field::new("/points:Color:colors", DataType::Utf8, true)
                    .with_metadata(component_metadata("/points", "colors")),
                Field::new("/cameras:Transform3D:transform", DataType::Utf8, true)
                    .with_metadata(component_metadata("/cameras", "transform")),
            ],
            HashMap::new(),
        ))
    }

    #[test]
    fn test_projection_single_entity() {
        let schema = make_schema_with_entities();
        // Select seg_id + log_time + both /points columns
        let projection = vec![0, 1, 2, 3];
        let paths = extract_projected_entity_paths(&schema, &projection, &[]);
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&EntityPath::from("/points")));
    }

    #[test]
    fn test_projection_multiple_entities() {
        let schema = make_schema_with_entities();
        // Select seg_id + one /points col + /cameras col
        let projection = vec![0, 2, 4];
        let paths = extract_projected_entity_paths(&schema, &projection, &[]);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&EntityPath::from("/points")));
        assert!(paths.contains(&EntityPath::from("/cameras")));
    }

    #[test]
    fn test_projection_only_non_entity_cols() {
        let schema = make_schema_with_entities();
        // Select only seg_id + log_time — no entity paths
        let projection = vec![0, 1];
        let paths = extract_projected_entity_paths(&schema, &projection, &[]);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_filter_adds_entity_paths() {
        use datafusion::logical_expr::col;

        let schema = make_schema_with_entities();
        // Project only /points column
        let projection = vec![0, 2];
        // Filter references /cameras column
        let filters = vec![col("/cameras:Transform3D:transform").is_not_null()];
        let paths = extract_projected_entity_paths(&schema, &projection, &filters);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&EntityPath::from("/points")));
        assert!(paths.contains(&EntityPath::from("/cameras")));
    }

    #[test]
    fn test_filter_with_non_entity_cols_only() {
        use datafusion::logical_expr::{col, lit};

        let schema = make_schema_with_entities();
        // Project only /points column
        let projection = vec![0, 2];
        // Filter references segment_id (no entity path) and time index (no entity path)
        let filters = vec![
            col("rerun_segment_id").eq(lit("seg_a")),
            col("log_time").gt(lit(100_i64)),
        ];
        let paths = extract_projected_entity_paths(&schema, &projection, &filters);
        // Only /points from projection — filters don't add entity paths
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&EntityPath::from("/points")));
    }

    #[test]
    fn test_component_projection_single() {
        let schema = make_schema_with_components();
        // Select seg_id + log_time + only the positions component of /points.
        let projection = vec![0, 1, 2];
        let components = extract_projected_components(&schema, &projection, &[]);
        assert_eq!(
            components,
            once("positions".to_owned()).collect::<BTreeSet<_>>(),
            "only the projected component should be selected, not its sibling `colors`",
        );
    }

    #[test]
    fn test_component_projection_skips_non_component_columns() {
        let schema = make_schema_with_components();
        // Select only seg_id + log_time — neither is a component column.
        let projection = vec![0, 1];
        let components = extract_projected_components(&schema, &projection, &[]);
        assert!(
            components.is_empty(),
            "segment-id and index columns must not be treated as components",
        );
    }

    #[test]
    fn test_component_projection_filter_adds_component() {
        use datafusion::logical_expr::col;

        let schema = make_schema_with_components();
        // Project only the positions component, but filter on a sibling component.
        let projection = vec![0, 2];
        let filters = vec![col("/points:Color:colors").is_not_null()];
        let components = extract_projected_components(&schema, &projection, &filters);
        assert_eq!(
            components,
            ["positions".to_owned(), "colors".to_owned()]
                .into_iter()
                .collect::<BTreeSet<_>>(),
            "a component referenced only by a filter must still be fetched",
        );
    }

    #[test]
    fn test_all_schema_components() {
        let schema = make_schema_with_components();
        assert_eq!(
            all_schema_components(&schema),
            [
                "positions".to_owned(),
                "colors".to_owned(),
                "transform".to_owned()
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            "every component column in the schema must be reported, deduped",
        );
    }

    #[test]
    fn test_component_projection_full_read_is_not_narrowed() {
        // A full read projects every column. The projected components then equal
        // the full schema set, so the scan must NOT narrow `fuzzy_descriptors`
        // (an exhaustive list would drop static-only components server-side).
        let schema = make_schema_with_components();
        let projection: Vec<usize> = (0..schema.fields().len()).collect();
        let projected = extract_projected_components(&schema, &projection, &[]);
        assert_eq!(
            projected,
            all_schema_components(&schema),
            "projecting all columns must reference every component",
        );
        assert!(
            projected.len() >= all_schema_components(&schema).len(),
            "a full projection is not a strict subset, so narrowing must be skipped",
        );
    }

    #[test]
    fn test_narrowing_intersects_with_original() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![
                EntityPath::from("/points"),
                EntityPath::from("/cameras"),
                EntityPath::from("/meshes"),
            ],
            select_all_entity_paths: false,
            ..Default::default()
        };

        query
            .entity_paths
            .retain(|path| projected_paths.contains(path));

        assert_eq!(query.entity_paths, vec![EntityPath::from("/points")]);
    }

    #[test]
    fn test_narrowing_empty_projected_no_change() {
        let projected_paths: BTreeSet<EntityPath> = BTreeSet::new();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/cameras")],
            select_all_entity_paths: false,
            ..Default::default()
        };
        let original = query.entity_paths.clone();

        // Empty projected_paths → caller should skip narrowing
        if !projected_paths.is_empty() {
            query
                .entity_paths
                .retain(|path| projected_paths.contains(path));
        }

        assert_eq!(query.entity_paths, original);
    }

    #[test]
    fn test_narrowing_select_all_no_change() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![],
            select_all_entity_paths: true,
            ..Default::default()
        };

        // select_all_entity_paths=true → skip narrowing
        if !query.select_all_entity_paths && !query.entity_paths.is_empty() {
            query
                .entity_paths
                .retain(|path| projected_paths.contains(path));
        }

        assert!(query.entity_paths.is_empty());
        assert!(query.select_all_entity_paths);
    }

    #[test]
    fn test_narrowing_preserves_multiple_queries() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut queries = vec![
            QueryDatasetRequest {
                entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/cameras")],
                select_all_entity_paths: false,
                ..Default::default()
            },
            QueryDatasetRequest {
                entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/meshes")],
                select_all_entity_paths: false,
                ..Default::default()
            },
        ];

        for query in &mut queries {
            if !query.select_all_entity_paths && !query.entity_paths.is_empty() {
                query
                    .entity_paths
                    .retain(|path| projected_paths.contains(path));
            }
        }

        assert_eq!(queries[0].entity_paths, vec![EntityPath::from("/points")]);
        assert_eq!(queries[1].entity_paths, vec![EntityPath::from("/points")]);
    }

    #[test]
    fn test_narrowing_skipped_with_fill_latest_at() {
        let projected_paths: BTreeSet<EntityPath> = once(EntityPath::from("/points")).collect();
        let mut query = QueryDatasetRequest {
            entity_paths: vec![EntityPath::from("/points"), EntityPath::from("/cameras")],
            select_all_entity_paths: false,
            ..Default::default()
        };
        let original = query.entity_paths.clone();

        // Simulate fill_latest_at=true check
        let sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;
        if sparse_fill_strategy == SparseFillStrategy::None && !projected_paths.is_empty() {
            query
                .entity_paths
                .retain(|path| projected_paths.contains(path));
        }

        assert_eq!(query.entity_paths, original);
    }
}
