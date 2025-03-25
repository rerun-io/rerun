//! Handles migrating old `re_types` to new ones.
//!
//!
use std::collections::BTreeMap;

use arrow::{
    array::{ArrayRef as ArrowArrayRef, RecordBatch as ArrowRecordBatch, RecordBatchOptions},
    datatypes::{Field as ArrowField, FieldRef as ArrowFieldRef, Schema as ArrowSchema},
};

/// Migrate old renamed types to new types.
pub fn migrate_record_batch(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    let num_columns = batch.num_columns();
    let mut fields: Vec<ArrowFieldRef> = Vec::with_capacity(num_columns);
    let mut columns: Vec<ArrowArrayRef> = Vec::with_capacity(num_columns);

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

    let schema = ArrowSchema::new_with_metadata(fields, batch.schema().metadata.clone());

    ArrowRecordBatch::try_new_with_options(
        schema.into(),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .expect("Can't fail")
}
