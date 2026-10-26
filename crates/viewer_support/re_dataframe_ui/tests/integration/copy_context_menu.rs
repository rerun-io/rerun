use crate::common;

use std::sync::Arc;

use arrow::array::{Int32Array, ListArray, StringArray, StructArray};
use arrow::datatypes::{DataType, Field, Fields, Int32Type, Schema};
use datafusion::prelude::SessionContext;
use egui::Modifiers;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use re_async::AsyncRuntimeHandle;
use re_dataframe_ui::{DataFusionTableWidget, TableBlueprints};
use re_test_context::TestContext;
use re_viewer_context::TableReference;

use common::run_async_harness;

/// Longer than the 100 characters the table shows of a string inside a list.
const LONG: &str = "s3://bucket/a-path-that-is-long-enough-to-be-truncated-in-the-table/episode_2026-02-11/derived_archetypes/recording.rrd";

#[tokio::test]
async fn test_copy_context_menu() {
    let (session_context, table_ref) = prepare_session_context();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([2400.0, 300.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &TableBlueprints::default(),
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;

    // A single string is copied as-is.
    let copied = copy_via_context_menu(&mut harness, "alpha", "Copy cell");
    assert_eq!(copied, "alpha");

    // The table shows only the first items of a list, but all of them are copied.
    let copied = copy_via_context_menu(&mut harness, "[1, 2, 3", "Copy cell");
    assert_eq!(copied, "[1, 2, 3, 4, 5, 6, 7, 8]");

    let copied = copy_via_context_menu(&mut harness, "alpha", "Copy row");
    assert_eq!(
        copied,
        format!(
            "alpha\t1\t[{LONG:?}, \"short\"]\t{{description: \"first\"}}\t[1, 2, 3, 4, 5, 6, 7, 8]"
        )
    );

    // Select the second row as well, then copy both.
    harness
        .get_by_label_contains("multi")
        .click_modifiers(Modifiers::COMMAND);
    harness.run_steps(2);
    let all_rows = format!(
        "Text\tNumber\tList\tStructured\tNumbers\n\
        alpha\t1\t[{LONG:?}, \"short\"]\t{{description: \"first\"}}\t[1, 2, 3, 4, 5, 6, 7, 8]\n\
        \"multi\n\"\"line\"\"\"\t2\t[]\t{{description: \"second\"}}\t[]"
    );
    let copied = copy_via_context_menu(&mut harness, "alpha", "Copy 2 selected rows");
    assert_eq!(copied, all_rows);
}

/// Right-click the cell whose label contains `cell_label`, click `menu_item`, and return what it
/// copied.
fn copy_via_context_menu(harness: &mut Harness<'_>, cell_label: &str, menu_item: &str) -> String {
    // The right-clicked cell is the one hovered in the frame before the click.
    harness.get_by_label_contains(cell_label).hover();
    harness.run_steps(2);
    harness.get_by_label_contains(cell_label).click_secondary();
    harness.run_steps(2);

    harness.get_by_label(menu_item).click();
    harness.run_steps(1);

    let copied = copied_texts(harness);
    harness.run_steps(2);

    assert_eq!(copied.len(), 1, "Expected one copy, got {copied:?}");
    copied.into_iter().next().unwrap_or_default()
}

/// The texts copied in the last frame.
fn copied_texts(harness: &Harness<'_>) -> Vec<String> {
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn prepare_session_context() -> (Arc<SessionContext>, &'static str) {
    let struct_fields = Fields::from(vec![Field::new("description", DataType::Utf8, false)]);
    let list_field = Arc::new(Field::new_list_field(DataType::Utf8, false));

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("text", DataType::Utf8, false),
            Field::new("number", DataType::Int32, false),
            Field::new("list", DataType::List(Arc::clone(&list_field)), false),
            Field::new("structured", DataType::Struct(struct_fields.clone()), false),
            Field::new(
                "numbers",
                DataType::List(Arc::new(Field::new_list_field(DataType::Int32, true))),
                false,
            ),
        ],
        Default::default(),
    ));

    let list_array = ListArray::new(
        list_field,
        arrow::buffer::OffsetBuffer::from_lengths([2, 0]),
        Arc::new(StringArray::from(vec![LONG, "short"])),
        None,
    );

    let struct_array = StructArray::new(
        struct_fields,
        vec![Arc::new(StringArray::from(vec!["first", "second"]))],
        None,
    );

    let numbers = ListArray::from_iter_primitive::<Int32Type, _, _>([
        Some((1..=8).map(Some).collect::<Vec<_>>()),
        Some(vec![]),
    ]);

    common::register_test_table(
        "test_table",
        schema,
        vec![
            Arc::new(StringArray::from(vec!["alpha", "multi\n\"line\""])),
            Arc::new(Int32Array::from(vec![1, 2])),
            Arc::new(list_array),
            Arc::new(struct_array),
            Arc::new(numbers),
        ],
    )
}
