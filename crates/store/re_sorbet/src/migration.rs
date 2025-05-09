//! Handles migrating old `re_types` to new ones.
//!
//!
use std::{collections::BTreeMap, sync::Arc};

use arrow::{
    array::{
        ArrayRef as ArrowArrayRef, AsArray as _, RecordBatch as ArrowRecordBatch,
        RecordBatchOptions,
    },
    datatypes::{Field as ArrowField, FieldRef as ArrowFieldRef, Schema as ArrowSchema},
};
use itertools::Itertools as _;
use re_log::ResultExt as _;
use re_tuid::Tuid;
use re_types_core::{Loggable as _, arrow_helpers::as_array_ref};

use crate::ColumnKind;

/// Migrate TUID:s with the pre-0.23 encoding.
pub fn migrate_tuids(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

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
pub fn migrate_record_batch(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    struct ArchetypeRename {
        new_name: &'static str,
        field_renames: BTreeMap<&'static str, &'static str>,
    }

    let archetype_renames = BTreeMap::from([
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
pub fn reorder_columns(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

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
    }

    ArrowRecordBatch::try_new_with_options(
        schema.clone(),
        arrays,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| ArrowRecordBatch::new_empty(schema))
}
