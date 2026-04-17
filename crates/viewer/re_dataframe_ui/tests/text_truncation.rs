mod common;

use std::sync::Arc;

use arrow::array::{Int32Array, RecordBatch, StringArray, StructArray};
use arrow::datatypes::{DataType, Field, Fields, Schema};
use datafusion::prelude::SessionContext;
use re_dataframe_ui::DataFusionTableWidget;
use re_test_context::TestContext;
use re_viewer_context::AsyncRuntimeHandle;

use common::run_async_harness;

#[tokio::test]
async fn test_text_truncation() {
    let (session_context, table_ref) = prepare_session_context();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([2500.0, 200.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                    .title("Text truncation")
                    .show(ctx, &runtime_handle, ui);
            });
        });

    run_async_harness(&mut harness).await;
    // TODO(rerun-io/egui_table#50): We should add a `max_default_width` field to egui_table
    // to truncate the root-level arrow strings
    harness.snapshot("test_text_truncation");
}

// ---

fn prepare_session_context() -> (Arc<SessionContext>, &'static str) {
    let lorem = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.";

    let struct_fields = Fields::from(vec![Field::new("description", DataType::Utf8, false)]);

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("text", DataType::Utf8, false),
            Field::new("number", DataType::Int32, false),
            Field::new("structured", DataType::Struct(struct_fields.clone()), false),
        ],
        Default::default(),
    ));

    let struct_array = StructArray::new(
        struct_fields,
        vec![Arc::new(StringArray::from(vec![lorem]))],
        None,
    );

    let batch = RecordBatch::try_new_with_options(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec![lorem])),
            Arc::new(Int32Array::from(vec![42])),
            Arc::new(struct_array),
        ],
        &Default::default(),
    )
    .expect("Failed to create a record batch");

    let session_context = Arc::new(SessionContext::new());
    let table_ref = "test_table";
    session_context
        .register_batch(table_ref, batch)
        .expect("Failed to register the table");

    (session_context, table_ref)
}
