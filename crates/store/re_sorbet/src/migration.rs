//! Handles migrating old `re_types` to new ones.
//!
//!
use std::{collections::BTreeMap, sync::Arc};

use arrow::{
    array::{
        ArrayRef as ArrowArrayRef, AsArray as _, RecordBatch as ArrowRecordBatch,
        RecordBatchOptions,
    },
    datatypes::{Field as ArrowField, FieldRef as ArrowFieldRef, Fields, Schema as ArrowSchema},
};
use itertools::Itertools as _;
use re_log::ResultExt as _;
use re_tuid::Tuid;
use re_types_core::{Loggable as _, arrow_helpers::as_array_ref};

use crate::ColumnKind;

/// Migrate TUID:s with the pre-0.23 encoding.
#[tracing::instrument(level = "trace", skip_all)]
pub fn migrate_tuids(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    let needs_migration = batch.schema_ref().fields().iter().any(|field| {
        field.extension_type_name() == Some("rerun.datatypes.TUID")
            || field.name() == "rerun.controls.RowId"
    });
    if !needs_migration {
        return batch.clone();
    }

    let num_columns = batch.num_columns();
    let mut fields: Vec<ArrowFieldRef> = Vec::with_capacity(num_columns);
    let mut columns: Vec<ArrowArrayRef> = Vec::with_capacity(num_columns);

    for (field, array) in itertools::izip!(batch.schema().fields(), batch.columns()) {
        let (mut field, mut array) = (field.clone(), array.clone());

        let is_tuid = field.extension_type_name() == Some("rerun.datatypes.TUID")
            || field.name() == "rerun.controls.RowId";
        if is_tuid {
            (field, array) = migrate_tuid_column(field, array);
        }

        fields.push(field);
        columns.push(array);
    }

    let schema = Arc::new(ArrowSchema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    ArrowRecordBatch::try_new_with_options(
        schema.clone(),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| ArrowRecordBatch::new_empty(schema))
}

/// Migrate TUID:s with the pre-0.23 encoding.
#[tracing::instrument(level = "trace", skip_all)]
fn migrate_tuid_column(
    field: ArrowFieldRef,
    array: ArrowArrayRef,
) -> (ArrowFieldRef, ArrowArrayRef) {
    re_tracing::profile_function!();

    if let Some(struct_array) = array.as_struct_opt() {
        // Maybe legacy struct (from Rerun 0.22 or earlier):
        let [nanos, counters] = struct_array.columns() else {
            return (field, array);
        };

        let Some(nanos) = nanos.as_primitive_opt::<arrow::datatypes::UInt64Type>() else {
            return (field, array);
        };

        let Some(counters) = counters.as_primitive_opt::<arrow::datatypes::UInt64Type>() else {
            return (field, array);
        };

        re_tracing::profile_function!();

        let tuids: Vec<Tuid> = itertools::izip!(nanos.values(), counters.values())
            .map(|(&nanos, &inc)| Tuid::from_nanos_and_inc(nanos, inc))
            .collect();

        let new_field = ArrowField::new(field.name(), Tuid::arrow_datatype(), false)
            .with_metadata(field.metadata().clone());
        let new_array = re_types_core::tuids_to_arrow(&tuids);

        re_log::debug_once!("Migrated legacy TUID encoding of column {}", field.name());

        (new_field.into(), as_array_ref(new_array))
    } else {
        (field, array)
    }
}

/// Migrate old renamed types to new types.
#[tracing::instrument(level = "trace", skip_all)]
pub fn migrate_record_batch(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    struct ArchetypeRename {
        new_name: &'static str,
        field_renames: BTreeMap<&'static str, &'static str>,
    }

    let archetype_renames = BTreeMap::from([
        // Deprecated in 0.23, removed in 0.24:
        (
            "rerun.archetypes.Scalar",
            ArchetypeRename {
                new_name: "rerun.archetypes.Scalars",
                field_renames: [("scalar", "scalars")].into(),
            },
        ),
        (
            "rerun.archetypes.SeriesLine",
            ArchetypeRename {
                new_name: "rerun.archetypes.SeriesLines",
                field_renames: [("color", "colors"), ("width", "widths"), ("name", "names")].into(),
            },
        ),
        (
            "rerun.archetypes.SeriesPoint",
            ArchetypeRename {
                new_name: "rerun.archetypes.SeriesPoints",
                field_renames: [
                    ("color", "colors"),
                    ("marker", "markers"),
                    ("name", "names"),
                    ("marker_size", "marker_sizes"),
                ]
                .into(),
            },
        ),
    ]);

    let needs_migration = batch.schema_ref().fields().iter().any(|field| {
        field
            .metadata()
            .get("rerun.archetype")
            .is_some_and(|arch| archetype_renames.contains_key(arch.as_str()))
    });
    if !needs_migration {
        return batch.clone();
    }

    let num_columns = batch.num_columns();
    let mut fields: Vec<ArrowFieldRef> = Vec::with_capacity(num_columns);
    let mut columns: Vec<ArrowArrayRef> = Vec::with_capacity(num_columns);

    for (field, array) in itertools::izip!(batch.schema().fields(), batch.columns()) {
        let mut metadata = field.metadata().clone();
        if let Some(archetype) = metadata.get_mut("rerun.archetype") {
            if let Some(archetype_rename) = archetype_renames.get(archetype.as_str()) {
                re_log::debug_once!(
                    "Migrating {archetype:?} to {:?}…",
                    archetype_rename.new_name
                );

                // Rename archetype:
                *archetype = archetype_rename.new_name.to_owned();

                // Renmame fields:
                if let Some(archetype_field) = metadata.get_mut("rerun.archetype_field") {
                    if let Some(new_field_name) =
                        archetype_rename.field_renames.get(archetype_field.as_str())
                    {
                        *archetype_field = (*new_field_name).to_owned();
                    }
                }
            }
        }

        let field = ArrowField::clone(field.as_ref())
            .clone()
            .with_metadata(metadata);

        fields.push(field.into());
        columns.push(array.clone());
    }

    let schema = Arc::new(ArrowSchema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    ArrowRecordBatch::try_new_with_options(
        schema.clone(),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| ArrowRecordBatch::new_empty(schema))
}

/// Put row-id first, then time columns, and last data columns.
#[tracing::instrument(level = "trace", skip_all)]
pub fn reorder_columns(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    let needs_reordering = 'check: {
        let mut row_ids = false;
        let mut indices = false;
        let mut components = false;

        let has_indices = batch.schema_ref().fields().iter().any(|field| {
            let column_kind = ColumnKind::try_from(field.as_ref()).unwrap_or(ColumnKind::Component);
            column_kind == ColumnKind::Index
        });

        for field in batch.schema_ref().fields() {
            let column_kind = ColumnKind::try_from(field.as_ref()).unwrap_or(ColumnKind::Component);
            match column_kind {
                ColumnKind::RowId => {
                    row_ids = true;
                    if (has_indices && indices) || components {
                        break 'check true;
                    }
                }

                ColumnKind::Index => {
                    indices = true;
                    if !row_ids || components {
                        break 'check true;
                    }
                }

                ColumnKind::Component => {
                    components = true;
                    if !row_ids || (has_indices && !indices) {
                        break 'check true;
                    }
                }
            }
        }

        false
    };

    if !needs_reordering {
        return batch.clone();
    }

    let mut row_ids = vec![];
    let mut indices = vec![];
    let mut components = vec![];

    for (field, array) in itertools::izip!(batch.schema().fields(), batch.columns()) {
        let field = field.clone();
        let array = array.clone();
        let column_kind = ColumnKind::try_from(field.as_ref()).unwrap_or(ColumnKind::Component);
        match column_kind {
            ColumnKind::RowId => row_ids.push((field, array)),
            ColumnKind::Index => indices.push((field, array)),
            ColumnKind::Component => components.push((field, array)),
        }
    }

    let (fields, arrays): (Vec<ArrowFieldRef>, Vec<ArrowArrayRef>) =
        itertools::chain!(row_ids, indices, components).unzip();

    let schema = Arc::new(ArrowSchema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    if schema.fields() != batch.schema().fields() {
        re_log::debug!(
            "Reordered columns. Before: {:?}, after: {:?}",
            batch
                .schema()
                .fields()
                .iter()
                .map(|f| f.name())
                .collect_vec(),
            schema.fields().iter().map(|f| f.name()).collect_vec()
        );
    } else {
        debug_assert!(
            false,
            "reordered something that didn't need to be reordered"
        );
    }

    ArrowRecordBatch::try_new_with_options(
        schema.clone(),
        arrays,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| ArrowRecordBatch::new_empty(schema))
}

/// Move indicator component to archetype field name.
// TODO(#8129): For now, this renames the indicator column metadata. Eventually, we want to remove the column altogether.
#[tracing::instrument(level = "trace", skip_all)]
pub fn rewire_indicator_components(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    let needs_rewiring = batch.schema_ref().fields().iter().any(|field| {
        let metadata = field.metadata();
        metadata
            .get("rerun.component")
            .is_some_and(|value| value.ends_with("Indicator"))
            || (!metadata.contains_key("rerun.archetype")
                && !metadata.contains_key("rerun.archetype_field"))
    });
    if !needs_rewiring {
        return batch.clone();
    }

    let fields = batch
        .schema()
        .fields()
        .into_iter()
        .map(|field| {
            let mut field = field.as_ref().clone();
            let mut metadata = field.metadata().clone();

            if let Some(value) = metadata.remove("rerun.component") {
                if value.ends_with("Indicator") {
                    re_log::debug_once!("Moving indicator to archetype field: {value}");
                    metadata.insert("rerun.archetype_field".to_owned(), value);

                    // We also strip the archetype name from any indicators.
                    // It turns out that too narrow indicator descriptors cause problems while querying.
                    // More information: <https://github.com/rerun-io/rerun/pull/9938#issuecomment-2888808593>
                    if let Some(archetype_name) = metadata.remove("rerun.archetype") {
                        re_log::debug_once!(
                            "Stripped archetype name from indicator: {archetype_name}"
                        );
                    }
                } else if !metadata.contains_key("rerun.archetype")
                    && !metadata.contains_key("rerun.archetype_field")
                {
                    // If we don't find the above keys, we likely encountered data that was logged via `AnyValues`.
                    // We do our best effort to convert that.
                    re_log::debug_once!("Moving stray component name {value:?} to archetype field");
                    metadata.insert("rerun.archetype_field".to_owned(), value);
                } else {
                    metadata.insert("rerun.component".to_owned(), value);
                }
            }
            field.set_metadata(metadata);
            Arc::new(field)
        })
        .collect::<Fields>();

    let schema = Arc::new(ArrowSchema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    ArrowRecordBatch::try_new_with_options(
        schema.clone(),
        batch.columns().to_vec(),
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| ArrowRecordBatch::new_empty(schema))
}
