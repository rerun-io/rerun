use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions};
use arrow::datatypes::{DataType, Field, Fields, Schema};
use egui::vec2;
use egui_kittest::{Harness, SnapshotResults};
use re_dataframe_ui::column_header_tooltip_ui;
use re_log_types::{EntityPath, Timeline};
use re_sorbet::{
    BatchType, ComponentColumnDescriptor, IndexColumnDescriptor, RowIdColumnDescriptor, SorbetBatch,
};
use re_types_core::reflection::generic_placeholder_for_datatype;
use re_types_core::{ArchetypeName, ComponentIdentifier, ComponentType};

#[test]
fn test_column_header_tooltips() {
    let mut snapshot_results = SnapshotResults::new();
    let fields_and_descriptions = test_fields();

    let fields = Fields::from(
        fields_and_descriptions
            .iter()
            .map(|(field, _)| field.clone())
            .collect::<Vec<_>>(),
    );

    let data = fields_and_descriptions
        .iter()
        .map(|(field, _)| Arc::new(generic_placeholder_for_datatype(field.data_type())) as ArrayRef)
        .collect::<Vec<_>>();

    let record_batch = RecordBatch::try_new_with_options(
        Arc::new(Schema::new_with_metadata(
            fields.clone(),
            Default::default(),
        )),
        data,
        &RecordBatchOptions::default(),
    )
    .expect("Failed to create test RecordBatch");

    let sorbet_batch = SorbetBatch::try_from_record_batch(&record_batch, BatchType::Dataframe)
        .expect("Failed to create test SorbetBatch");

    let descriptions = fields_and_descriptions
        .iter()
        .map(|(_, name)| *name)
        .collect::<Vec<_>>();

    for (desc, field, migrated_field, description) in itertools::izip!(
        sorbet_batch.sorbet_schema().columns.clone(),
        &fields,
        sorbet_batch.fields(),
        descriptions
    ) {
        for show_extras in [false, true] {
            let mut harness = Harness::builder()
                .with_size(vec2(600.0, 600.0))
                .build_ui(|ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());

                    column_header_tooltip_ui(
                        ui,
                        &(&desc).into(),
                        field.as_ref(),
                        migrated_field.as_ref(),
                        show_extras,
                    );
                });

            harness.run();
            harness.snapshot(format!(
                "header_tooltip_{description}{}",
                if show_extras { "_with_extras" } else { "" }
            ));

            snapshot_results.extend_harness(&mut harness);
        }
    }
}

fn test_fields() -> Vec<(Field, &'static str)> {
    let component_column_desc = ComponentColumnDescriptor {
        store_datatype: DataType::Int64,
        component_type: Some(ComponentType::new("rerun.components.Null")),
        entity_path: EntityPath::from("/some/path"),
        archetype: Some(ArchetypeName::new("ArchetypeName")),
        component: ComponentIdentifier::from("component_identifier"),
        is_static: false,
        is_tombstone: false,
        is_semantically_empty: false,
    };

    vec![
        (
            RowIdColumnDescriptor::from_sorted(true).to_arrow_field(),
            "row_id_column",
        ),
        (
            IndexColumnDescriptor::from(Timeline::new_duration("duration_timeline"))
                .to_arrow_field(),
            "index_column",
        ),
        (
            component_column_desc.to_arrow_field(BatchType::Chunk),
            "chunk_component_column",
        ),
        (
            component_column_desc.to_arrow_field(BatchType::Dataframe),
            "dataframe_component_column",
        ),
        (
            Field::new("simple_field", DataType::Int64, false),
            "raw_field",
        ),
        (
            Field::new("simple_field", DataType::Int64, true),
            "row_field_nullable",
        ),
        (
            Field::new("user_metadata", DataType::Float32, false)
                .with_metadata(std::iter::once(("hello".to_owned(), "world".to_owned())).collect()),
            "raw_field_user_metadata",
        ),
        (
            Field::new("user_metadata", DataType::Float32, false).with_metadata(
                [
                    ("hello".to_owned(), "world".to_owned()),
                    (
                        re_sorbet::metadata::SORBET_ENTITY_PATH.to_owned(),
                        "/entity/path".to_owned(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            "raw_field_user_and_sorbet_metadata",
        ),
    ]
}
