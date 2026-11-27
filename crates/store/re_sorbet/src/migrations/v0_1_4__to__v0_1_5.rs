use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Field, FieldRef, Schema};
use re_log::ResultExt as _;

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 1, 4);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 5);

    fn migrate(batch: RecordBatch) -> RecordBatch {
        drop_time_panel_play_state(batch)
    }

    fn migrate_rrd_manifest(batch: RecordBatch) -> RecordBatch {
        drop_time_panel_play_state(batch)
    }
}

const PLAY_STATE_COMPONENT: &str = "TimePanelBlueprint:play_state";

fn is_play_state_field(field: &Field) -> bool {
    field
        .metadata()
        .get("rerun:component")
        .is_some_and(|component| component == PLAY_STATE_COMPONENT)
}

/// Drop the `TimePanelBlueprint:play_state` blueprint component.
///
/// This works for both chunks and the per-component index columns of an RRD manifest.
#[tracing::instrument(level = "trace", skip_all)]
fn drop_time_panel_play_state(batch: RecordBatch) -> RecordBatch {
    if !batch
        .schema()
        .fields()
        .iter()
        .any(|field| is_play_state_field(field))
    {
        return batch;
    }

    re_log::debug_once!("Dropping {PLAY_STATE_COMPONENT} columns");

    let (schema, columns, row_count) = batch.into_parts();

    let (new_fields, new_columns): (Vec<FieldRef>, Vec<ArrayRef>) =
        std::iter::zip(schema.fields().iter(), columns)
            .filter(|(field, _)| !is_play_state_field(field))
            .map(|(field, column)| (field.clone(), column))
            .unzip();

    let new_schema = Arc::new(Schema::new_with_metadata(
        new_fields,
        schema.metadata().clone(),
    ));

    RecordBatch::try_new_with_options(
        new_schema.clone(),
        new_columns,
        &RecordBatchOptions::default().with_row_count(Some(row_count)),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(new_schema))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow::array::{Int64Array, ListArray};
    use arrow::datatypes::{DataType, UInt8Type};

    use super::*;

    /// A blueprint chunk has a `TimePanelBlueprint:play_state` column next to another column.
    /// The migration drops the play state column and keeps the other one unchanged.
    #[test]
    fn play_state_column_is_dropped() {
        let play_state = Arc::new(ListArray::from_iter_primitive::<UInt8Type, _, _>([Some([
            Some(1),
        ])])) as ArrayRef;
        let play_state_field =
            Field::new(PLAY_STATE_COMPONENT, play_state.data_type().clone(), true).with_metadata(
                HashMap::from([
                    (
                        "rerun:archetype".to_owned(),
                        "rerun.blueprint.archetypes.TimePanelBlueprint".to_owned(),
                    ),
                    (
                        "rerun:component".to_owned(),
                        PLAY_STATE_COMPONENT.to_owned(),
                    ),
                    (
                        "rerun:component_type".to_owned(),
                        "rerun.blueprint.components.PlayState".to_owned(),
                    ),
                ]),
            );
        let other_field = Field::new("other", DataType::Int64, false);
        let other = Arc::new(Int64Array::from(vec![7])) as ArrayRef;

        let batch = RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(
                vec![play_state_field, other_field.clone()],
                HashMap::default(),
            )),
            vec![play_state, other.clone()],
            &RecordBatchOptions::default().with_row_count(Some(1)),
        )
        .unwrap();

        let migrated = drop_time_panel_play_state(batch);

        let schema = migrated.schema();
        assert_eq!(schema.fields().len(), 1);
        assert_eq!(schema.field(0), &other_field);
        assert_eq!(migrated.column(0), &other);
        assert_eq!(migrated.num_rows(), 1);
    }
}
