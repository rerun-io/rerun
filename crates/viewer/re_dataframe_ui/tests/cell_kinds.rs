//! Test configured cell kinds in table and card layouts.

mod common;

use std::sync::Arc;

use arrow::array::{Array as _, BooleanArray, Int32Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::prelude::SessionContext;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_async::AsyncRuntimeHandle;
use re_chunk_store::external::re_chunk::Chunk;
use re_dataframe_ui::{DataFusionTableWidget, TableBlueprints};
use re_log_types::{StoreId, StoreKind};
use re_sdk_types::blueprint::archetypes::{CardLayout, TableColumn, TableLayout};
use re_sdk_types::blueprint::components::TableCellKind;
use re_sdk_types::components::Blob;
use re_test_context::TestContext;
use re_types_core::{Component as _, ToArrow as _};
use re_viewer_context::{TableReference, blueprint_timepoint_for_writes};

use common::run_async_harness;

/// Exercise forced cell renderers and flag editing in table and card layouts.
#[test]
fn test_forced_cell_kinds() {
    // Thumbnail rendering requires a main-thread token, while editable cells require a
    // multi-threaded Tokio runtime for blocking credential access.
    let test_thread = std::thread::Builder::new()
        .name("main".to_owned())
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread() // NOLINT: owned by this dedicated test thread
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(run_forced_cell_kinds_test());
        })
        .expect("test thread should start");

    if let Err(payload) = test_thread.join() {
        std::panic::resume_unwind(payload);
    }
}

async fn run_forced_cell_kinds_test() {
    let (session_context, table_ref) = setup_cell_kind_table();

    // Editing requires a remote table reference, while the test data remains in-memory.
    let remote_uri: re_uri::EntryUri = "rerun+http://localhost:1234/entry/1"
        .parse()
        .expect("test entry URI should be valid");
    let mut test_context = TestContext::new();
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
        .expect("test should run inside its Tokio runtime");
    let table_blueprints =
        setup_cell_kind_blueprint(&test_context, TableReference::from(remote_uri.clone()));

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([1600.0, 600.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::from(remote_uri.clone()),
                )
                .title("Forced cell kinds")
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &table_blueprints,
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;

    // The table snapshot shows all forced renderers together, including multiple flag columns.
    harness.get_by_label("Table view").click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("forced_cell_kinds_table");

    let table_flags = harness
        .query_all_by_role_and_label(Role::CheckBox, "Flag")
        .collect::<Vec<_>>();
    assert_eq!(table_flags.len(), 9, "three flag columns for three rows");
    assert_eq!(
        table_flags
            .iter()
            .filter(|flag| flag.accesskit_node().is_disabled())
            .count(),
        3,
        "the read-only flag column must remain disabled"
    );
    table_flags
        .into_iter()
        .find(|flag| !flag.accesskit_node().is_disabled())
        .expect("editable table flag should be present")
        .click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("forced_cell_kinds_table_edited");

    // Cards use only the first flag in their header and omit all flags from labeled fields.
    harness.get_by_label("Cards view").click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("forced_cell_kinds_cards");

    let card_flags = harness
        .query_all_by_role_and_label(Role::CheckBox, "Flag")
        .collect::<Vec<_>>();
    assert_eq!(card_flags.len(), 3, "cards show only the first flag field");
    assert!(
        card_flags
            .iter()
            .all(|flag| !flag.accesskit_node().is_disabled()),
        "the selected card flag is editable"
    );
    card_flags[0].click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("forced_cell_kinds_cards_edited");
}

#[expect(clippy::needless_pass_by_value)]
fn setup_cell_kind_blueprint(
    test_context: &TestContext,
    table_ref: TableReference,
) -> TableBlueprints {
    let blueprint_id = StoreId::random(StoreKind::Blueprint, "cell-kind-test");
    let mut store_hub = test_context.store_hub.lock();
    let store = store_hub.store_bundle_mut().blueprint_entry(&blueprint_id);
    let timepoint = blueprint_timepoint_for_writes(store);
    let field_order = [
        "name",
        "regular_bool",
        "editable_bool",
        "editable_flag",
        "readonly_flag",
        "second_flag",
        "thumbnail",
        "link",
        "entry_kind",
        "invalid_editable",
    ];

    for chunk in [
        Chunk::builder("table/layouts/table")
            .with_archetype_auto_row(
                timepoint.clone(),
                &TableLayout::new().with_column_order(field_order),
            )
            .build()
            .expect("table layout chunk should build"),
        Chunk::builder("table/layouts/cards")
            .with_archetype_auto_row(
                timepoint.clone(),
                &CardLayout::new(field_order).with_title("name"),
            )
            .build()
            .expect("card layout chunk should build"),
    ] {
        store
            .add_chunk(&Arc::new(chunk))
            .expect("layout chunk should be added");
    }

    let columns = [
        ("name", "Name", TableCellKind::Auto, false),
        ("regular_bool", "Boolean", TableCellKind::Auto, false),
        (
            "editable_bool",
            "Editable boolean",
            TableCellKind::Auto,
            true,
        ),
        ("editable_flag", "Editable flag", TableCellKind::Flag, true),
        (
            "readonly_flag",
            "Read-only flag",
            TableCellKind::Flag,
            false,
        ),
        ("second_flag", "Second flag", TableCellKind::Flag, true),
        ("thumbnail", "Thumbnail", TableCellKind::Thumbnail, false),
        ("link", "Link", TableCellKind::Link, false),
        ("entry_kind", "Entry kind", TableCellKind::EntryKind, false),
        (
            "invalid_editable",
            "Invalid editable link",
            TableCellKind::Auto,
            true,
        ),
    ];
    for layout in ["table/layouts/table/columns", "table/layouts/cards/fields"] {
        for &(source, name, cell_kind, editable) in &columns {
            let chunk = Arc::new(
                Chunk::builder(format!("{layout}/{source}"))
                    .with_archetype_auto_row(
                        timepoint.clone(),
                        &TableColumn::new()
                            .with_name(name)
                            .with_cell_kind(cell_kind)
                            .with_editable(editable),
                    )
                    .build()
                    .expect("column chunk should build"),
            );
            store
                .add_chunk(&chunk)
                .expect("column chunk should be added");
        }
    }

    let mut table_blueprints = TableBlueprints::default();
    table_blueprints
        .set_default_blueprint(&table_ref, &blueprint_id, &mut store_hub)
        .expect("default table blueprint should be set");
    table_blueprints
}

fn setup_cell_kind_table() -> (Arc<SessionContext>, &'static str) {
    let image_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/assets/image/grinda.jpg");
    let image = Blob::from(std::fs::read(&image_path).unwrap_or_else(|err| {
        panic!(
            "Failed to read test image: {err}\nFile path: {}",
            image_path.display()
        )
    }));
    let images = Blob::to_arrow([image.clone(), image.clone(), image])
        .expect("test images should serialize");
    let thumbnail_field = Field::new("thumbnail", images.data_type().clone(), false).with_metadata(
        [
            (
                re_sorbet::metadata::RERUN_KIND.to_owned(),
                re_sorbet::ColumnKind::Component.to_string(),
            ),
            (
                re_types_core::FIELD_METADATA_KEY_COMPONENT.to_owned(),
                "thumbnail".to_owned(),
            ),
            (
                re_types_core::FIELD_METADATA_KEY_COMPONENT_TYPE.to_owned(),
                Blob::name().to_string(),
            ),
        ]
        .into(),
    );
    let (thumbnail_field, thumbnails) = re_arrow_util::wrap_in_list_array(&thumbnail_field, images);
    let id_field = Field::new("id", DataType::Int64, false).with_metadata(
        [(
            re_sorbet::metadata::SORBET_IS_TABLE_INDEX.to_owned(),
            "true".to_owned(),
        )]
        .into(),
    );
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            id_field,
            Field::new("name", DataType::Utf8, false),
            Field::new("regular_bool", DataType::Boolean, false),
            Field::new("editable_bool", DataType::Boolean, true),
            Field::new("editable_flag", DataType::Boolean, true),
            Field::new("readonly_flag", DataType::Boolean, true),
            Field::new("second_flag", DataType::Boolean, false),
            thumbnail_field,
            Field::new("link", DataType::Utf8, false),
            Field::new("entry_kind", DataType::Int32, false),
            Field::new("invalid_editable", DataType::Utf8, false),
        ],
        Default::default(),
    ));
    common::register_test_table(
        "cell_kind_table",
        schema,
        vec![
            Arc::new(arrow::array::Int64Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec!["Alice", "Bob", "Charlie"])),
            Arc::new(BooleanArray::from(vec![true, false, true])),
            Arc::new(BooleanArray::from(vec![Some(false), None, Some(true)])),
            Arc::new(BooleanArray::from(vec![Some(false), None, Some(true)])),
            Arc::new(BooleanArray::from(vec![Some(true), Some(false), None])),
            Arc::new(BooleanArray::from(vec![false, true, false])),
            Arc::new(thumbnails),
            Arc::new(StringArray::from(vec![
                "rerun+http://localhost:1234/entry/1",
                "rerun+http://localhost:1234/entry/2",
                "rerun+http://localhost:1234/entry/3",
            ])),
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec!["one", "two", "three"])),
        ],
    )
}
