//! Interpret an arbitrary Arrow record batch as Rerun chunk data.
//!
//! This is the core implementation for the Arrow → chunk interpretation that the SDKs and the
//! platform share. It is surfaced in Python through `Chunk.from_record_batch` and
//! `rr.send_dataframe`.

use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef as ArrowArrayRef, RecordBatch as ArrowRecordBatch, RecordBatchOptions,
};
use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};

use re_arrow_util::RecordBatchExt as _;
use re_log_types::{EntityPath, TimelineName};
use re_types_core::{ChunkId, FIELD_METADATA_KEY_COMPONENT, Loggable as _, RowId};

use crate::{
    BatchType, ChunkBatch, ComponentColumnDescriptor, IndexColumnDescriptor, MetadataExt as _,
    RowIdColumnDescriptor, SorbetBatch, SorbetError, SorbetSchema,
    metadata::{RERUN_CHUNK_ID, RERUN_KIND, SORBET_ENTITY_PATH, SORBET_INDEX_NAME},
};

/// The Arrow field metadata key that holds the extension name (e.g. the TUID extension).
const ARROW_EXTENSION_NAME: &str = "ARROW:extension:name";

/// How index (timeline) columns are chosen when interpreting a dataframe batch.
#[derive(Clone, Debug)]
pub enum DataframeIndex {
    /// Derive index columns from `rerun:kind`/`rerun:index_name` metadata; error if none found.
    Auto,

    /// Promote exactly these columns to timelines; all other non-row-id columns become components.
    Columns(Vec<TimelineName>),

    /// No timelines (static data).
    Static,
}

/// Errors raised while interpreting a dataframe record batch as [`ChunkBatch`]es.
#[derive(thiserror::Error, Debug)]
pub enum DataframeToChunksError {
    /// An underlying sorbet-level failure: classification, list-wrapping, assembly, or an
    /// unsupported index datatype ([`crate::IndexColumnError::UnsupportedTimeType`]).
    #[error(transparent)]
    Sorbet(#[from] SorbetError),

    /// `index` was left at the default but the batch carries no index metadata, so it cannot be
    /// told apart from static data.
    #[error(
        "The record batch carries no index column, so it cannot be unambiguously interpreted as \
         temporal or static. Pass `index=<column>` for temporal data or `index=None` for static \
         data."
    )]
    AmbiguousStaticData,

    /// `index=None` (static) was requested, but the batch contradicts it with index metadata.
    #[error(
        "`index=None` (static) was requested, but the record batch also carries index metadata or \
         names an index column. Drop the index metadata, or request a temporal interpretation \
         instead."
    )]
    StaticWithIndex,

    /// A named index column does not exist in the batch.
    #[error("The requested index column {0:?} is not present in the record batch.")]
    MissingIndexColumn(String),

    /// A column promoted to an index contains null values.
    #[error(
        "The index column {0:?} contains null values. Time columns must be dense — express static \
         data with `index=None` rather than null times. (Mixing static and temporal rows in a \
         single batch is not supported.)"
    )]
    NullIndexColumn(String),

    /// The batch has no component columns, so there is nothing to log.
    #[error("The record batch contains no component columns, so there is nothing to log.")]
    NoComponentColumns,

    /// An identified chunk (row-id column + chunk id) resolves to more than one entity path.
    #[error(
        "The record batch is an identified chunk (it carries a row-id column and a chunk id), but \
         resolves to more than one entity path. An identified chunk is preserved as-is and must be \
         a single chunk for a single entity. To reinterpret it into one chunk per entity (with \
         freshly-minted ids), drop the chunk-id metadata and/or the row-id column."
    )]
    IdentifiedChunkWithMultipleEntities,
}

/// Interpret an arbitrary Arrow record batch as one [`ChunkBatch`] per entity path.
///
/// Each column is classified as a row-id column, an index (timeline) column, or a component column.
/// Component columns are grouped by entity path, and one [`ChunkBatch`] is emitted per distinct
/// entity path, in first-seen column order.
///
/// `rerun:*` Arrow metadata, when present, drives the classification of each column, as well as the
/// entity path / archetype / component / component-type of component columns.
///
/// # Chunk identity
///
/// A row-id column together with a `rerun:id` chunk id mark the batch as a *fully identified* chunk
/// (e.g. one produced from a [`ChunkBatch`]). Both the row ids and chunk id are preserved when:
/// - both are present in the input batch,
/// - `index` is [`DataframeIndex::Auto`] (the default), and
/// - `entity_path` is not set.
///
/// Such an identified chunk round-trips into a single chunk, and must resolve to a single entity
/// path (otherwise [`DataframeToChunksError::IdentifiedChunkWithMultipleEntities`]). A row-id column
/// is recognized by a `rerun:kind` of `row_id`/`control`, or the `rerun.datatypes.TUID` Arrow
/// extension.
///
/// If any of these conditions is not met, the batch is either not fully identified, or its data is
/// being reinterpreted (`index`) and/or relocated (`entity_path`). In that case, fresh row ids and
/// a fresh chunk id are minted and the input ones discarded to avoid unwanted reuse of UUID. Also,
/// the data may be spread into one chunk per entity path.
///
/// # Index (timeline) columns
///
/// The `index` argument ([`DataframeIndex`]) selects which columns become timelines. By default
/// ([`DataframeIndex::Auto`]), index columns are derived from metadata. The timeline type is
/// derived from the column's datatype:
/// - `Int64` → sequence,
/// - `Timestamp(ns)` → timestamp
/// - `Duration(ns)` → duration
///
/// Any other datatype is rejected.
///
/// Index columns are shared across every emitted chunk, and must be dense: a null index value is
/// rejected, since it would make a row neither temporal nor static (see *Limitations*).
///
/// # Component columns and entity paths
///
/// Every non-row-id, non-index column is a component column. Component arrays may be either lists
/// (one component batch per row) or plain arrays; plain arrays are automatically wrapped as
/// single-element lists.
///
/// A component's entity path is resolved, in order, from:
/// - its own `rerun:entity_path` metadata,
/// - the batch-level `rerun:entity_path` metadata,
/// - the column-name convention (see below),
/// - the `entity_path` argument, if provided,
/// - the root entity (`/`).
///
/// ## Column-name convention
///
/// When a component column has no `rerun:entity_path` metadata and its name starts with `/` and
/// contains a `:`, the part before the first `:` is taken as the entity path and the remainder as
/// the component identifier (e.g. `/points:Points3D:positions` → entity `/points`, component
/// `Points3D:positions`). Names without a leading `/` are not split and land on the resolved
/// default entity.
///
/// # Static data
///
/// With [`DataframeIndex::Static`] — or under [`DataframeIndex::Auto`] when the batch is an
/// already-identified static chunk that round-trips as-is (see *Chunk identity*) — the resulting
/// chunks have no timeline. (A non-identified batch with no index metadata cannot be assumed static
/// under [`DataframeIndex::Auto`]; it is ambiguous and rejected with
/// [`DataframeToChunksError::AmbiguousStaticData`] — pass `index=None` to force a static reading.)
///
/// Static chunks with more than one row can be legitimate in some cases, but latest-at queries only
/// surface the last row, so an info-level message is emitted in that case. (Only when a chunk is
/// freshly assembled — an already-identified chunk that is preserved as-is is passed through without
/// this check.)
///
/// # Limitations
///
/// * A batch that mixes static and temporal rows — i.e. one with some `null` index values — is not
///   split into a mix of static and temporal chunks. Such a batch is rejected outright (a null
///   index value yields [`DataframeToChunksError::NullIndexColumn`]).
/// * Recording-property columns (named `property:…`, mapping to the `/__properties` entity) are not
///   recognized by the column-name convention.
// NOTE: Agent, keep this in sync with `Chunk.from_record_batch`.
pub fn chunk_batches_from_dataframe_record_batch(
    batch: &ArrowRecordBatch,
    index: &DataframeIndex,
    entity_path: Option<&EntityPath>,
) -> Result<Vec<ChunkBatch>, DataframeToChunksError> {
    re_tracing::profile_function!();

    // Step 0: chunk-identity dispatch.
    let has_row_id = batch
        .schema_ref()
        .fields()
        .iter()
        .any(|f| is_row_id_field(f));
    let has_chunk_id = batch.schema_ref().metadata().contains_key(RERUN_CHUNK_ID);
    let preserve_requested = matches!(index, DataframeIndex::Auto) && entity_path.is_none();

    if has_row_id && has_chunk_id && preserve_requested {
        // Identified chunk: preserve its identity, round-tripping into a single chunk.
        return preserve_identified_chunk(batch).map(|cb| vec![cb]);
    }

    // Otherwise we mint a fresh identity. Drop any provided row-id column so it is neither carried
    // as a component nor mistaken for the minted one (the input chunk id is likewise ignored — the
    // assembly step stamps a freshly-minted `rerun:id` per chunk).
    let batch = drop_row_id_columns(batch)?;

    // Step 1: pre-stamp a working copy of the schema metadata.
    let stamped = stamp_dataframe_metadata(&batch, index, entity_path)?;

    // Step 2: classify (this is where a bad index dtype raises `UnsupportedTimeType`).
    let sorbet_batch = SorbetBatch::try_from_record_batch(&stamped, BatchType::Dataframe)?;

    // Step 3: policy.
    let index_columns: Vec<(&IndexColumnDescriptor, &ArrowArrayRef)> =
        sorbet_batch.index_columns().collect();
    let component_columns: Vec<(&ComponentColumnDescriptor, &ArrowArrayRef)> =
        sorbet_batch.component_columns().collect();

    if matches!(index, DataframeIndex::Auto) && index_columns.is_empty() {
        return Err(DataframeToChunksError::AmbiguousStaticData);
    }
    if component_columns.is_empty() {
        return Err(DataframeToChunksError::NoComponentColumns);
    }
    // The index columns are shared across every emitted chunk, so validate them once here.
    reject_null_index_columns(index_columns.iter().map(|&(descr, array)| (descr, array)))?;

    // Step 4: group component columns by entity path, preserving first-seen order.
    let mut entity_order: Vec<EntityPath> = Vec::new();
    for (descr, _) in &component_columns {
        if !entity_order.contains(&descr.entity_path) {
            entity_order.push(descr.entity_path.clone());
        }
    }

    // Step 5: assemble one chunk batch per entity group.
    let num_rows = batch.num_rows();
    let mut chunk_batches = Vec::with_capacity(entity_order.len());
    for entity in entity_order {
        let group: Vec<(&ComponentColumnDescriptor, &ArrowArrayRef)> = component_columns
            .iter()
            .filter(|(descr, _)| descr.entity_path == entity)
            .copied()
            .collect();

        let chunk_batch = assemble_chunk_batch(&entity, num_rows, &index_columns, &group)?;

        // Step 6: note static chunks with more than one row. Occasionally legit (tf-transforms),
        // but worth a heads-up
        if chunk_batch.is_static() && chunk_batch.num_rows() > 1 {
            re_log::info!(
                "Building a static chunk for entity {entity} from {} rows (latest-at queries only \
                surface the last row)",
                chunk_batch.num_rows()
            );
        }

        chunk_batches.push(chunk_batch);
    }

    Ok(chunk_batches)
}

/// Reject any index (time) column that contains nulls.
///
/// A null index value belongs to a row that is neither temporal nor static — exactly the mixed
/// static/temporal case we do not (yet) handle. Rather than emit an unsound chunk batch and defer
/// the failure to chunk construction, we refuse it here.
fn reject_null_index_columns<'a>(
    index_columns: impl IntoIterator<Item = (&'a IndexColumnDescriptor, &'a ArrowArrayRef)>,
) -> Result<(), DataframeToChunksError> {
    for (descr, array) in index_columns {
        if array.null_count() > 0 {
            return Err(DataframeToChunksError::NullIndexColumn(
                descr.column_name().to_owned(),
            ));
        }
    }
    Ok(())
}

/// Is this field a row-id column?
///
/// A field with `rerun:kind ∈ {row_id, control}` or the TUID Arrow extension.
fn is_row_id_field(field: &ArrowField) -> bool {
    matches!(field.get_opt(RERUN_KIND), Some("row_id" | "control"))
        || field.get_opt(ARROW_EXTENSION_NAME) == Some(re_tuid::Tuid::ARROW_EXTENSION_NAME)
}

/// Was this field detected as a row-id column *only* via the TUID extension (no `rerun:kind`)?
fn is_row_id_via_extension_only(field: &ArrowField) -> bool {
    !matches!(field.get_opt(RERUN_KIND), Some("row_id" | "control"))
        && field.get_opt(ARROW_EXTENSION_NAME) == Some(re_tuid::Tuid::ARROW_EXTENSION_NAME)
}

/// Is this field an index (timeline) column under `Auto` classification?
fn is_index_field_auto(field: &ArrowField) -> bool {
    matches!(field.get_opt(RERUN_KIND), Some("index" | "time"))
        || field.get_opt(SORBET_INDEX_NAME).is_some()
}

/// Split a column name following the `/entity:component` convention.
///
/// Returns `(entity, component)` if `name` starts with `/` and contains a `:`.
fn split_name_convention(name: &str) -> Option<(&str, &str)> {
    if name.starts_with('/') {
        name.split_once(':')
    } else {
        None
    }
}

/// Resolve a component column's entity path *for the multi-entity guard* on the preserve path.
///
/// Deliberately ignores the batch-level entity path and the `entity_path` argument: it only flags
/// genuinely-different per-column entities, which `ChunkBatch::try_from` (keyed by batch-level
/// entity) would otherwise silently collapse.
fn guard_component_entity(field: &ArrowField) -> EntityPath {
    if let Some(entity) = field.get_opt(SORBET_ENTITY_PATH) {
        EntityPath::parse_forgiving(entity)
    } else if let Some((entity, _component)) = split_name_convention(field.name()) {
        EntityPath::parse_forgiving(entity)
    } else {
        EntityPath::root()
    }
}

/// Return a copy of `batch` with any row-id column(s) removed.
fn drop_row_id_columns(
    batch: &ArrowRecordBatch,
) -> Result<ArrowRecordBatch, DataframeToChunksError> {
    batch
        .clone()
        .filter_columns_by(|field| !is_row_id_field(field))
        .map_err(|err| DataframeToChunksError::Sorbet(err.into()))
}

/// The preserve path: round-trip an *identified chunk* (row-id column + chunk id) as-is.
///
/// The caller guarantees the batch is fully identified and that no reinterpretation/relocation was
/// requested ([`DataframeIndex::Auto`], no `entity_path`); the only remaining requirement is that
/// it resolves to a single entity path.
fn preserve_identified_chunk(
    batch: &ArrowRecordBatch,
) -> Result<ChunkBatch, DataframeToChunksError> {
    // Single-entity requirement: resolve per-column entities of the component columns (everything
    // that is neither a row-id nor an index column).
    let mut entities: Vec<EntityPath> = Vec::new();
    for field in batch.schema_ref().fields() {
        if is_row_id_field(field) || is_index_field_auto(field) {
            continue;
        }
        let entity = guard_component_entity(field);
        if !entities.contains(&entity) {
            entities.push(entity);
        }
    }
    if entities.len() > 1 {
        return Err(DataframeToChunksError::IdentifiedChunkWithMultipleEntities);
    }

    // Single entity (or no components): preserve. Stamp the metadata-safety bits when absent.
    let mut fields: Vec<ArrowField> = Vec::with_capacity(batch.num_columns());
    for field in batch.schema_ref().fields() {
        let mut field = field.as_ref().clone();
        // If the row-id column was detected only via the TUID extension, give it an explicit kind
        // so the chunk classifier recognizes it.
        if is_row_id_via_extension_only(&field) {
            field
                .metadata_mut()
                .insert(RERUN_KIND.to_owned(), "control".to_owned());
        }
        fields.push(field);
    }

    let mut batch_metadata = batch.schema_ref().metadata().clone();
    // Stamp the entity path when absent (the single resolved entity, else root).
    batch_metadata
        .entry(SORBET_ENTITY_PATH.to_owned())
        .or_insert_with(|| {
            entities
                .first()
                .cloned()
                .unwrap_or_else(EntityPath::root)
                .to_string()
        });
    // Stamp the version so we skip the migration chain (and its metadata rewrites).
    batch_metadata
        .entry(SorbetSchema::METADATA_KEY_VERSION.to_owned())
        .or_insert_with(|| SorbetSchema::METADATA_VERSION.to_string());

    let stamped = rebuild_record_batch(batch, fields, batch_metadata)?;
    let chunk_batch = ChunkBatch::try_from(&stamped)?;
    reject_null_index_columns(chunk_batch.index_columns())?;
    Ok(chunk_batch)
}

/// Rebuild a record batch with new field metadata / batch metadata but the same arrays.
fn rebuild_record_batch(
    batch: &ArrowRecordBatch,
    fields: Vec<ArrowField>,
    batch_metadata: HashMap<String, String>,
) -> Result<ArrowRecordBatch, SorbetError> {
    let schema = Arc::new(ArrowSchema::new_with_metadata(fields, batch_metadata));
    Ok(ArrowRecordBatch::try_new_with_options(
        schema,
        batch.columns().to_vec(),
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )?)
}

/// Build a working copy of the batch with stamped Rerun metadata, ready for Dataframe classification.
fn stamp_dataframe_metadata(
    batch: &ArrowRecordBatch,
    index: &DataframeIndex,
    entity_path: Option<&EntityPath>,
) -> Result<ArrowRecordBatch, DataframeToChunksError> {
    let batch_entity = batch
        .schema_ref()
        .metadata()
        .get(SORBET_ENTITY_PATH)
        .cloned();

    // For `Columns`, verify every named column exists.
    if let DataframeIndex::Columns(names) = index {
        for name in names {
            if !batch
                .schema_ref()
                .fields()
                .iter()
                .any(|f| f.name() == name.as_str())
            {
                return Err(DataframeToChunksError::MissingIndexColumn(name.to_string()));
            }
        }
    }

    let mut fields: Vec<ArrowField> = Vec::with_capacity(batch.num_columns());
    for field in batch.schema_ref().fields() {
        let mut field = field.as_ref().clone();
        let name = field.name().clone();

        // Static contradiction check (on the raw field metadata).
        if matches!(index, DataframeIndex::Static)
            && (matches!(field.get_opt(RERUN_KIND), Some("index" | "time"))
                || field.get_opt(SORBET_INDEX_NAME).is_some())
        {
            return Err(DataframeToChunksError::StaticWithIndex);
        }

        let is_index = match index {
            DataframeIndex::Auto => is_index_field_auto(&field),
            DataframeIndex::Columns(names) => names.iter().any(|n| n.as_str() == name),
            DataframeIndex::Static => false,
        };

        if is_index {
            field
                .metadata_mut()
                .insert(RERUN_KIND.to_owned(), "index".to_owned());
            field
                .metadata_mut()
                .entry(SORBET_INDEX_NAME.to_owned())
                .or_insert_with(|| name.clone());
        } else {
            // Component column. Make the kind explicit.
            field
                .metadata_mut()
                .insert(RERUN_KIND.to_owned(), "data".to_owned());

            // Resolve the entity path (field metadata → batch metadata → name convention → arg →
            // root) and stamp it so per-column classification picks it up.
            if !field.metadata().contains_key(SORBET_ENTITY_PATH) {
                let resolved = if let Some(batch_entity) = &batch_entity {
                    batch_entity.clone()
                } else if let Some((entity, component)) = split_name_convention(&name) {
                    if !field.metadata().contains_key(FIELD_METADATA_KEY_COMPONENT) {
                        field.metadata_mut().insert(
                            FIELD_METADATA_KEY_COMPONENT.to_owned(),
                            component.to_owned(),
                        );
                    }
                    entity.to_owned()
                } else if let Some(entity_path) = entity_path {
                    entity_path.to_string()
                } else {
                    EntityPath::root().to_string()
                };
                field
                    .metadata_mut()
                    .insert(SORBET_ENTITY_PATH.to_owned(), resolved);
            }
        }

        fields.push(field);
    }

    let mut batch_metadata = batch.schema_ref().metadata().clone();
    // Stamp the version so the migration chain early-outs instead of rewriting reserved metadata.
    batch_metadata
        .entry(SorbetSchema::METADATA_KEY_VERSION.to_owned())
        .or_insert_with(|| SorbetSchema::METADATA_VERSION.to_string());

    Ok(rebuild_record_batch(batch, fields, batch_metadata)?)
}

/// Assemble a single chunk batch from a minted row-id column, the shared index columns, and one
/// entity group's component columns.
fn assemble_chunk_batch(
    entity: &EntityPath,
    num_rows: usize,
    index_columns: &[(&IndexColumnDescriptor, &ArrowArrayRef)],
    components: &[(&ComponentColumnDescriptor, &ArrowArrayRef)],
) -> Result<ChunkBatch, SorbetError> {
    let mut fields: Vec<ArrowField> =
        Vec::with_capacity(1 + index_columns.len() + components.len());
    let mut arrays: Vec<ArrowArrayRef> = Vec::with_capacity(fields.capacity());

    // Minted row-id column (sequential ids are sorted by construction).
    fields.push(RowIdColumnDescriptor::from_sorted(true).to_arrow_field());
    arrays.push(Arc::new(mint_row_ids(num_rows)));

    // Shared index columns.
    for (descr, array) in index_columns {
        fields.push(descr.to_arrow_field());
        arrays.push((*array).clone());
    }

    // This group's component columns (carrying their classified descriptors).
    for (descr, array) in components {
        fields.push(descr.to_arrow_field(BatchType::Chunk));
        arrays.push((*array).clone());
    }

    let batch_metadata = HashMap::from([
        (RERUN_CHUNK_ID.to_owned(), ChunkId::new().to_string()),
        (SORBET_ENTITY_PATH.to_owned(), entity.to_string()),
        (
            SorbetSchema::METADATA_KEY_VERSION.to_owned(),
            SorbetSchema::METADATA_VERSION.to_string(),
        ),
    ]);

    let schema = Arc::new(ArrowSchema::new_with_metadata(fields, batch_metadata));
    let record_batch = ArrowRecordBatch::try_new_with_options(
        schema,
        arrays,
        &RecordBatchOptions::default().with_row_count(Some(num_rows)),
    )?;

    // `try_from` (not `try_new`) so plain component arrays get auto list-wrapped + reordered.
    ChunkBatch::try_from(&record_batch)
}

/// Mint `count` fresh, sequential (hence sorted) row ids as a `FixedSizeBinary(16)` array.
fn mint_row_ids(count: usize) -> arrow::array::FixedSizeBinaryArray {
    let mut ids = Vec::with_capacity(count);
    let mut next = RowId::new();
    for _ in 0..count {
        ids.push(next);
        next = next.next();
    }
    re_log::debug_assert_eq!(
        RowId::arrow_datatype(),
        arrow::datatypes::DataType::FixedSizeBinary(16)
    );
    RowId::arrow_from_slice(&ids)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{
        ArrayRef as ArrowArrayRef, DurationNanosecondArray, Float32Array, Int64Array,
        RecordBatch as ArrowRecordBatch, RecordBatchOptions, TimestampMicrosecondArray,
        TimestampNanosecondArray,
    };
    use arrow::datatypes::{
        DataType as ArrowDatatype, Field as ArrowField, Schema as ArrowSchema, TimeUnit,
    };
    use re_log_types::{EntityPath, TimelineName};
    use re_types_core::{Loggable as _, RowId};

    use super::{DataframeIndex, chunk_batches_from_dataframe_record_batch};
    use crate::{
        ChunkBatch, DataframeToChunksError, RowIdColumnDescriptor, SorbetError, SorbetSchema,
        metadata::{RERUN_CHUNK_ID, RERUN_KIND, SORBET_ENTITY_PATH, SORBET_INDEX_NAME},
    };

    fn field(name: &str, dt: ArrowDatatype, meta: &[(&str, &str)]) -> ArrowField {
        ArrowField::new(name, dt, true).with_metadata(
            meta.iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        )
    }

    fn batch(
        fields: Vec<ArrowField>,
        arrays: Vec<ArrowArrayRef>,
        batch_meta: &[(&str, &str)],
    ) -> ArrowRecordBatch {
        let num_rows = arrays.first().map_or(0, |a| a.len());
        let schema = Arc::new(ArrowSchema::new_with_metadata(
            fields,
            batch_meta
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        ));
        ArrowRecordBatch::try_new_with_options(
            schema,
            arrays,
            &RecordBatchOptions::default().with_row_count(Some(num_rows)),
        )
        .unwrap()
    }

    fn int64(values: &[i64]) -> ArrowArrayRef {
        Arc::new(Int64Array::from(values.to_vec()))
    }

    fn floats(values: &[f32]) -> ArrowArrayRef {
        Arc::new(Float32Array::from(values.to_vec()))
    }

    /// `kind=index` + `kind=data` → one temporal chunk.
    #[test]
    fn auto_temporal() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "/e:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/e")],
                ),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert!(!chunk.is_static());
        assert_eq!(chunk.entity_path(), &EntityPath::from("/e"));
        assert_eq!(chunk.index_columns().count(), 1);
        assert_eq!(
            chunk.index_columns().next().unwrap().0.timeline_name(),
            TimelineName::from("frame")
        );
    }

    /// An `index_name`-only column (no `rerun:kind`) is still promoted to a timeline under Auto.
    /// Guards the `reader()` round-trip.
    #[test]
    fn auto_index_name_only() {
        let rb = batch(
            vec![
                field(
                    "frame",
                    ArrowDatatype::Int64,
                    &[(SORBET_INDEX_NAME, "frame")],
                ),
                field(
                    "/e:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/e")],
                ),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].is_static());
        assert_eq!(chunks[0].index_columns().count(), 1);
    }

    /// A plain (non-list) component array is auto list-wrapped.
    #[test]
    fn plain_component_array_is_list_wrapped() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "/e:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/e")],
                ),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        let (_descr, array) = chunks[0].component_columns().next().unwrap();
        assert!(
            matches!(array.data_type(), ArrowDatatype::List(_)),
            "component column should be list-wrapped, got {:?}",
            array.data_type()
        );
    }

    /// Auto + zero index columns → `AmbiguousStaticData`.
    #[test]
    fn auto_no_index_is_ambiguous() {
        let rb = batch(
            vec![field(
                "/e:c",
                ArrowDatatype::Float32,
                &[(SORBET_ENTITY_PATH, "/e")],
            )],
            vec![floats(&[1.0, 2.0])],
            &[],
        );
        let err = chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None)
            .unwrap_err();
        assert!(matches!(err, DataframeToChunksError::AmbiguousStaticData));
    }

    /// `Columns` promotes the named columns, with their time type taken from the Arrow dtype.
    #[test]
    fn columns_promotion_time_types() {
        let cases: Vec<(ArrowDatatype, ArrowArrayRef)> = vec![
            (ArrowDatatype::Int64, int64(&[0, 1])),
            (
                ArrowDatatype::Timestamp(TimeUnit::Nanosecond, None),
                Arc::new(TimestampNanosecondArray::from(vec![0, 1])),
            ),
            (
                ArrowDatatype::Duration(TimeUnit::Nanosecond),
                Arc::new(DurationNanosecondArray::from(vec![0, 1])),
            ),
        ];
        for (dt, array) in cases {
            let rb = batch(
                vec![
                    field("t", dt.clone(), &[]),
                    field(
                        "/e:c",
                        ArrowDatatype::Float32,
                        &[(SORBET_ENTITY_PATH, "/e")],
                    ),
                ],
                vec![array, floats(&[1.0, 2.0])],
                &[],
            );
            let chunks = chunk_batches_from_dataframe_record_batch(
                &rb,
                &DataframeIndex::Columns(vec![TimelineName::from("t")]),
                None,
            )
            .unwrap();
            assert_eq!(chunks.len(), 1, "dtype {dt:?}");
            assert_eq!(chunks[0].index_columns().count(), 1, "dtype {dt:?}");
        }
    }

    /// `Columns` on an unsupported time dtype fails at classification with `UnsupportedTimeType`.
    #[test]
    fn columns_bad_time_type() {
        let rb = batch(
            vec![
                field(
                    "t",
                    ArrowDatatype::Timestamp(TimeUnit::Microsecond, None),
                    &[],
                ),
                field(
                    "/e:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/e")],
                ),
            ],
            vec![
                Arc::new(TimestampMicrosecondArray::from(vec![0, 1])),
                floats(&[1.0, 2.0]),
            ],
            &[],
        );
        let err = chunk_batches_from_dataframe_record_batch(
            &rb,
            &DataframeIndex::Columns(vec![TimelineName::from("t")]),
            None,
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                DataframeToChunksError::Sorbet(SorbetError::IndexColumn(_))
            ),
            "got {err}"
        );
    }

    /// A named index column that does not exist → `MissingIndexColumn`.
    #[test]
    fn columns_missing() {
        let rb = batch(
            vec![field(
                "/e:c",
                ArrowDatatype::Float32,
                &[(SORBET_ENTITY_PATH, "/e")],
            )],
            vec![floats(&[1.0, 2.0])],
            &[],
        );
        let err = chunk_batches_from_dataframe_record_batch(
            &rb,
            &DataframeIndex::Columns(vec![TimelineName::from("nope")]),
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, DataframeToChunksError::MissingIndexColumn(_)),
            "got {err}"
        );
    }

    /// A null value in a promoted index column → `NullIndexColumn` (rejected eagerly).
    #[test]
    fn null_index_value_rejected() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "/e:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/e")],
                ),
            ],
            vec![
                Arc::new(Int64Array::from(vec![Some(0_i64), None])),
                floats(&[1.0, 2.0]),
            ],
            &[],
        );
        let err = chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None)
            .unwrap_err();
        assert!(
            matches!(err, DataframeToChunksError::NullIndexColumn(_)),
            "got {err}"
        );
    }

    /// `Static` produces a static chunk; an index contradiction is rejected.
    #[test]
    fn static_and_contradiction() {
        let rb = batch(
            vec![field(
                "/e:c",
                ArrowDatatype::Float32,
                &[(SORBET_ENTITY_PATH, "/e")],
            )],
            vec![floats(&[1.0])],
            &[],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Static, None).unwrap();
        assert!(chunks[0].is_static());
        assert_eq!(chunks[0].index_columns().count(), 0);

        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "/e:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/e")],
                ),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[],
        );
        let err = chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Static, None)
            .unwrap_err();
        assert!(
            matches!(err, DataframeToChunksError::StaticWithIndex),
            "got {err}"
        );
    }

    /// Multiple entities split into multiple chunks, preserving first-seen column order.
    #[test]
    fn multi_entity_split_order() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "/b:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/b")],
                ),
                field(
                    "/a:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/a")],
                ),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0]), floats(&[3.0, 4.0])],
            &[],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        let entities: Vec<_> = chunks.iter().map(|c| c.entity_path().to_string()).collect();
        assert_eq!(entities, vec!["/b".to_owned(), "/a".to_owned()]);

        // The shared index column is carried by every emitted chunk.
        for chunk in &chunks {
            assert_eq!(chunk.index_columns().count(), 1);
            assert_eq!(
                chunk.index_columns().next().unwrap().0.timeline_name(),
                TimelineName::from("frame")
            );
        }
    }

    /// Batch-level `rerun:entity_path` wins over the column-name convention (resolution-order guard).
    #[test]
    fn batch_level_entity_path_wins_over_name_convention() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                // A conventional name that *would* resolve to `/a`, but no own entity metadata.
                field("/a:c", ArrowDatatype::Float32, &[]),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[(SORBET_ENTITY_PATH, "/batch")],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].entity_path(), &EntityPath::from("/batch"));
    }

    /// The name convention sets the component identifier to the part after the first `:`.
    #[test]
    fn name_convention_extracts_component_id() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field("/points:Points3D:positions", ArrowDatatype::Float32, &[]),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        assert_eq!(chunks[0].entity_path(), &EntityPath::from("/points"));
        let (descr, _) = chunks[0].component_columns().next().unwrap();
        assert_eq!(descr.component.to_string(), "Points3D:positions");
    }

    /// The column-name convention: leading-`/` required, otherwise root.
    #[test]
    fn name_convention() {
        let cases = [
            ("/e:c", "/e"),
            ("/e:Arch:c", "/e"),
            ("foo:bar", "/"),      // no leading slash → root
            ("property:foo", "/"), // not recognized → root
        ];
        for (name, expected_entity) in cases {
            let rb = batch(
                vec![
                    field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                    field(name, ArrowDatatype::Float32, &[]),
                ],
                vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
                &[],
            );
            let chunks =
                chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None)
                    .unwrap();
            assert_eq!(
                chunks[0].entity_path(),
                &EntityPath::from(expected_entity),
                "name {name:?}"
            );
        }
    }

    /// `entity_path` arg is used as the default for un-located component columns.
    #[test]
    fn entity_path_arg_default() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field("bare", ArrowDatatype::Float32, &[]),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[],
        );
        let chunks = chunk_batches_from_dataframe_record_batch(
            &rb,
            &DataframeIndex::Auto,
            Some(&EntityPath::from("/world")),
        )
        .unwrap();
        assert_eq!(chunks[0].entity_path(), &EntityPath::from("/world"));
    }

    /// Zero component columns → `NoComponentColumns`.
    #[test]
    fn no_component_columns() {
        let rb = batch(
            vec![field(
                "frame",
                ArrowDatatype::Int64,
                &[(RERUN_KIND, "index")],
            )],
            vec![int64(&[0, 1])],
            &[],
        );
        let err = chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None)
            .unwrap_err();
        assert!(
            matches!(err, DataframeToChunksError::NoComponentColumns),
            "got {err}"
        );
    }

    /// Build a chunk-shaped batch carrying a row-id column with a known chunk id + row ids.
    fn row_id_batch(
        component_meta: &[(&str, &str)],
        with_version: bool,
        entity: &str,
    ) -> (ArrowRecordBatch, re_types_core::ChunkId, RowId) {
        let chunk_id = re_types_core::ChunkId::new();
        let first_row_id = RowId::new();
        let row_ids = RowId::arrow_from_slice(&[first_row_id, first_row_id.next()]);

        let mut batch_meta = vec![
            (RERUN_CHUNK_ID.to_owned(), chunk_id.to_string()),
            (SORBET_ENTITY_PATH.to_owned(), entity.to_owned()),
        ];
        if with_version {
            batch_meta.push((
                SorbetSchema::METADATA_KEY_VERSION.to_owned(),
                SorbetSchema::METADATA_VERSION.to_string(),
            ));
        }

        let schema = Arc::new(ArrowSchema::new_with_metadata(
            vec![
                RowIdColumnDescriptor::from_sorted(true).to_arrow_field(),
                field("c", ArrowDatatype::Float32, component_meta),
            ],
            batch_meta.into_iter().collect(),
        ));
        let rb = ArrowRecordBatch::try_new_with_options(
            schema,
            vec![Arc::new(row_ids), floats(&[1.0, 2.0])],
            &RecordBatchOptions::default().with_row_count(Some(2)),
        )
        .unwrap();
        (rb, chunk_id, first_row_id)
    }

    /// The preserve path keeps the chunk id and row ids.
    #[test]
    fn preserve_keeps_ids() {
        let (rb, chunk_id, first_row_id) = row_id_batch(&[(RERUN_KIND, "data")], true, "/foo");
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_id(), chunk_id);
        assert_eq!(chunks[0].entity_path(), &EntityPath::from("/foo"));
        let (_descr, ids) = chunks[0].row_id_column();
        let preserved = RowId::from_arrow(&(Arc::new(ids.clone()) as ArrowArrayRef)).unwrap();
        assert_eq!(preserved[0], first_row_id);
    }

    /// A TUID-extension-only row-id column (no `rerun:kind`) preserves correctly.
    #[test]
    fn preserve_tuid_extension_only() {
        let chunk_id = re_types_core::ChunkId::new();
        let mut row_id_field = RowIdColumnDescriptor::from_sorted(true).to_arrow_field();
        // Drop the `rerun:kind` so only the TUID extension marks it.
        row_id_field.metadata_mut().remove(RERUN_KIND);
        let schema = Arc::new(ArrowSchema::new_with_metadata(
            vec![
                row_id_field,
                field("c", ArrowDatatype::Float32, &[(RERUN_KIND, "data")]),
            ],
            [
                (RERUN_CHUNK_ID.to_owned(), chunk_id.to_string()),
                (SORBET_ENTITY_PATH.to_owned(), "/foo".to_owned()),
                (
                    SorbetSchema::METADATA_KEY_VERSION.to_owned(),
                    SorbetSchema::METADATA_VERSION.to_string(),
                ),
            ]
            .into_iter()
            .collect(),
        ));
        let rb = ArrowRecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(RowId::arrow_from_slice(&[RowId::new(), RowId::new()])),
                floats(&[1.0, 2.0]),
            ],
            &RecordBatchOptions::default().with_row_count(Some(2)),
        )
        .unwrap();
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_id(), chunk_id);
    }

    /// Reinterpreting an identified chunk (e.g. `index=None`) mints a fresh identity and discards
    /// the provided chunk id / row ids.
    #[test]
    fn reinterpretation_mints_fresh_identity() {
        let (rb, chunk_id, first_row_id) = row_id_batch(&[(RERUN_KIND, "data")], true, "/foo");
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Static, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].is_static());
        assert_eq!(chunks[0].entity_path(), &EntityPath::from("/foo"));

        // The provided identity was discarded: fresh chunk id and fresh row ids.
        assert_ne!(chunks[0].chunk_id(), chunk_id);
        let (_descr, ids) = chunks[0].row_id_column();
        let minted = RowId::from_arrow(&(Arc::new(ids.clone()) as ArrowArrayRef)).unwrap();
        assert_ne!(minted[0], first_row_id);
    }

    /// Providing `entity_path` defeats identity preservation: even a fully-identified chunk is
    /// reinterpreted, minting a fresh chunk id and fresh row ids.
    #[test]
    fn entity_path_arg_forces_mint() {
        let chunk_id = re_types_core::ChunkId::new();
        let first_row_id = RowId::new();
        let schema = Arc::new(ArrowSchema::new_with_metadata(
            vec![
                RowIdColumnDescriptor::from_sorted(true).to_arrow_field(),
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "c",
                    ArrowDatatype::Float32,
                    &[(RERUN_KIND, "data"), (SORBET_ENTITY_PATH, "/foo")],
                ),
            ],
            [
                (RERUN_CHUNK_ID.to_owned(), chunk_id.to_string()),
                (SORBET_ENTITY_PATH.to_owned(), "/foo".to_owned()),
                (
                    SorbetSchema::METADATA_KEY_VERSION.to_owned(),
                    SorbetSchema::METADATA_VERSION.to_string(),
                ),
            ]
            .into_iter()
            .collect(),
        ));
        let rb = ArrowRecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(RowId::arrow_from_slice(&[
                    first_row_id,
                    first_row_id.next(),
                ])),
                int64(&[0, 1]),
                floats(&[1.0, 2.0]),
            ],
            &RecordBatchOptions::default().with_row_count(Some(2)),
        )
        .unwrap();

        // Sanity: with no `entity_path` this is the preserve path and keeps the chunk id.
        let preserved =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        assert_eq!(preserved[0].chunk_id(), chunk_id);

        // With `entity_path`, identity preservation is defeated → fresh chunk id and row ids.
        let chunks = chunk_batches_from_dataframe_record_batch(
            &rb,
            &DataframeIndex::Auto,
            Some(&EntityPath::from("/relocated")),
        )
        .unwrap();
        assert_eq!(chunks.len(), 1);
        assert_ne!(chunks[0].chunk_id(), chunk_id);
        let (_descr, ids) = chunks[0].row_id_column();
        let minted = RowId::from_arrow(&(Arc::new(ids.clone()) as ArrowArrayRef)).unwrap();
        assert_ne!(minted[0], first_row_id);
    }

    /// A row-id column but no chunk id is only *partially* identified, so fresh ids are minted and
    /// the batch may split into one chunk per entity path.
    #[test]
    fn partial_identity_mints_and_splits() {
        let schema = Arc::new(ArrowSchema::new_with_metadata(
            vec![
                RowIdColumnDescriptor::from_sorted(true).to_arrow_field(),
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "x",
                    ArrowDatatype::Float32,
                    &[(RERUN_KIND, "data"), (SORBET_ENTITY_PATH, "/a")],
                ),
                field(
                    "y",
                    ArrowDatatype::Float32,
                    &[(RERUN_KIND, "data"), (SORBET_ENTITY_PATH, "/b")],
                ),
            ],
            // Note: no `rerun:id` → only partially identified.
            std::iter::once((
                SorbetSchema::METADATA_KEY_VERSION.to_owned(),
                SorbetSchema::METADATA_VERSION.to_string(),
            ))
            .collect(),
        ));
        let rb = ArrowRecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(RowId::arrow_from_slice(&[RowId::new(), RowId::new()])),
                int64(&[0, 1]),
                floats(&[1.0, 2.0]),
                floats(&[3.0, 4.0]),
            ],
            &RecordBatchOptions::default().with_row_count(Some(2)),
        )
        .unwrap();
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        let entities: Vec<_> = chunks.iter().map(|c| c.entity_path().to_string()).collect();
        assert_eq!(entities, vec!["/a".to_owned(), "/b".to_owned()]);
    }

    /// An identified chunk (row-id + chunk id) with components on more than one entity → error.
    #[test]
    fn identified_chunk_with_multiple_entities() {
        let chunk_id = re_types_core::ChunkId::new();
        let schema = Arc::new(ArrowSchema::new_with_metadata(
            vec![
                RowIdColumnDescriptor::from_sorted(true).to_arrow_field(),
                field(
                    "x",
                    ArrowDatatype::Float32,
                    &[(RERUN_KIND, "data"), (SORBET_ENTITY_PATH, "/a")],
                ),
                field(
                    "y",
                    ArrowDatatype::Float32,
                    &[(RERUN_KIND, "data"), (SORBET_ENTITY_PATH, "/b")],
                ),
            ],
            [
                (RERUN_CHUNK_ID.to_owned(), chunk_id.to_string()),
                (
                    SorbetSchema::METADATA_KEY_VERSION.to_owned(),
                    SorbetSchema::METADATA_VERSION.to_string(),
                ),
            ]
            .into_iter()
            .collect(),
        ));
        let rb = ArrowRecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(RowId::arrow_from_slice(&[RowId::new(), RowId::new()])),
                floats(&[1.0, 2.0]),
                floats(&[3.0, 4.0]),
            ],
            &RecordBatchOptions::default().with_row_count(Some(2)),
        )
        .unwrap();
        let err = chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None)
            .unwrap_err();
        assert!(
            matches!(
                err,
                DataframeToChunksError::IdentifiedChunkWithMultipleEntities
            ),
            "got {err}"
        );
    }

    /// The preserve path stamps `sorbet:version`, so a version-less row-id batch carrying a
    /// `Pose*` component type survives the migration chain unchanged.
    #[test]
    fn preserve_migration_safety() {
        let (rb, _, _) = row_id_batch(
            &[
                (RERUN_KIND, "data"),
                ("rerun:component", "translation"),
                ("rerun:component_type", "rerun.components.PoseTranslation3D"),
            ],
            /* with_version = */ false,
            "/foo",
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        let (descr, _) = chunks[0].component_columns().next().unwrap();
        assert_eq!(
            descr.component_type.map(|c| c.to_string()),
            Some("rerun.components.PoseTranslation3D".to_owned()),
            "the Pose component type should not have been migrated away"
        );
    }

    /// Sanity: a `ChunkBatch` round-trips back through `ArrowRecordBatch` (smoke test for assembly).
    #[test]
    fn assembly_is_a_valid_chunk_batch() {
        let rb = batch(
            vec![
                field("frame", ArrowDatatype::Int64, &[(RERUN_KIND, "index")]),
                field(
                    "/e:c",
                    ArrowDatatype::Float32,
                    &[(SORBET_ENTITY_PATH, "/e")],
                ),
            ],
            vec![int64(&[0, 1]), floats(&[1.0, 2.0])],
            &[],
        );
        let chunks =
            chunk_batches_from_dataframe_record_batch(&rb, &DataframeIndex::Auto, None).unwrap();
        let round_trip = ArrowRecordBatch::from(&chunks[0]);
        assert!(ChunkBatch::try_from(&round_trip).is_ok());
    }
}
