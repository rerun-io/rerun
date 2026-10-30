//! Test configured component types in table and card layouts.

use crate::common;

use std::sync::Arc;

use arrow::array::{Array as _, BooleanArray, Float64Array, StringArray, UInt8Array};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::prelude::SessionContext;
use egui::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_async::AsyncRuntimeHandle;
use re_chunk_store::external::re_chunk::Chunk;
use re_dataframe_ui::{DataFusionTableWidget, TableBlueprints};
use re_log_types::{EntryName, StoreId, StoreKind};
use re_sdk_types::blueprint::archetypes::{CardLayout, TableColumn, TableLayout};
use re_sdk_types::components::{Blob, EntryKind, Flag, Thumbnail, Url};
use re_test_context::TestContext;
use re_types_core::{Component as _, ComponentType, ToArrow as _};
use re_viewer_context::{
    LinkKind, ResolvedEntry, TableReference, UrlNameLookup, blueprint_timepoint_for_writes,
    make_url_decorator,
};

use common::run_async_harness;

/// Values of the read-only `Url` column.
const LINKS: [&str; 3] = [
    "rerun+http://localhost:1234/entry/00000000000000000000000000000001",
    "rerun+http://localhost:1234/entry/00000000000000000000000000000002",
    "rerun+http://localhost:1234/entry/00000000000000000000000000000003",
];

/// Values of the editable column whose component type, `Url`, has no editor.
const EDITABLE_LINKS: [&str; 3] = [
    "rerun+http://localhost:1234/entry/00000000000000000000000000000004",
    "rerun+http://localhost:1234/entry/00000000000000000000000000000005",
    "rerun+http://localhost:1234/entry/00000000000000000000000000000006",
];

/// Values of the column whose component type has no UI in the Viewer.
const UNKNOWN_TYPE_VALUES: [&str; 3] = ["first unknown", "second unknown", "third unknown"];

/// The name the Viewer shows for the entry at `index` of `LINKS` followed by `EDITABLE_LINKS`.
fn entry_name(index: usize) -> String {
    format!("entry-{}", index + 1)
}

/// Resolves the entries the link columns refer to, as the catalog does in the Viewer.
fn entry_name_lookup() -> Arc<UrlNameLookup> {
    let lookup = std::iter::chain(&LINKS, &EDITABLE_LINKS)
        .enumerate()
        .map(|(index, link)| {
            let uri: re_uri::EntryUri = link.parse().expect("test entry URI should be valid");
            let resolved = ResolvedEntry {
                name: EntryName::new(entry_name(index)).expect("test entry name should be valid"),
                kind: LinkKind::Dataset,
            };
            ((uri.origin, uri.entry_id), resolved)
        })
        .collect();
    Arc::new(lookup)
}

/// Exercise component types forced by the blueprint and flag editing in table and card layouts.
#[test]
fn test_forced_component_types() {
    // Thumbnail rendering requires a main-thread token, while editable cells require a
    // multi-threaded Tokio runtime for blocking credential access.
    let test_thread = std::thread::Builder::new()
        .name("main".to_owned())
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread() // NOLINT: owned by this dedicated test thread
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(run_forced_component_types_test());
        })
        .expect("test thread should start");

    if let Err(payload) = test_thread.join() {
        std::panic::resume_unwind(payload);
    }
}

async fn run_forced_component_types_test() {
    let (session_context, table_ref) = setup_component_type_table();

    // Editing requires a remote table reference, while the test data remains in-memory.
    let remote_uri: re_uri::EntryUri =
        "rerun+http://localhost:1234/entry/00000000000000000000000000000001"
            .parse()
            .expect("test entry URI should be valid");
    let mut test_context = TestContext::new();
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
        .expect("test should run inside its Tokio runtime");
    let table_blueprints =
        setup_component_type_blueprint(&test_context, TableReference::from(remote_uri.clone()));
    let entry_name_lookup = entry_name_lookup();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([2000.0, 600.0])
        .build_ui(|ui| {
            re_ui::UrlDecorator::set(
                ui.ctx(),
                make_url_decorator(Arc::clone(&entry_name_lookup), ui.ctx().theme()),
            );
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::from(remote_uri.clone()),
                )
                .title("Forced component types")
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

    // The table snapshot shows all forced component types together, including multiple flag columns.
    harness.get_by_label("Table view").click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("forced_component_types_table");

    // An editable column whose component type has no editor shows its values read-only.
    for index in LINKS.len()..LINKS.len() + EDITABLE_LINKS.len() {
        harness.get_by_role_and_label(Role::Button, &entry_name(index));
    }
    assert_eq!(
        harness
            .query_all_by_role_and_label(Role::TextInput, "editable_link")
            .count(),
        0,
        "the editable link column has no text inputs"
    );

    // A column whose component type has no UI shows its values with the fallback UI.
    for value in UNKNOWN_TYPE_VALUES {
        harness.get_by_label_contains(value);
    }

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

    // Boolean columns without a component type are shown as checkboxes.
    let table_checkboxes = harness
        .query_all_by_role(Role::CheckBox)
        .filter(|checkbox| checkbox.accesskit_node().label().as_deref() != Some("Flag"))
        .collect::<Vec<_>>();
    assert_eq!(
        table_checkboxes.len(),
        6,
        "two checkbox columns for three rows"
    );
    assert_eq!(
        table_checkboxes
            .iter()
            .filter(|checkbox| checkbox.accesskit_node().is_disabled())
            .count(),
        3,
        "the read-only boolean column must remain disabled"
    );
    table_checkboxes
        .into_iter()
        .find(|checkbox| !checkbox.accesskit_node().is_disabled())
        .expect("editable table checkbox should be present")
        .click();
    run_async_harness(&test_context, &mut harness).await;

    // Editable string columns without a component type are edited as text.
    let text_cell = harness
        .query_all_by_role_and_label(Role::TextInput, "editable_text")
        .find(|node| node.value().as_deref() == Some("one"))
        .expect("editable text cell should be present");
    text_cell.focus();
    text_cell.type_text(" edited");
    harness.key_press(egui::Key::Enter);
    run_async_harness(&test_context, &mut harness).await;
    assert!(
        harness
            .query_all_by_role_and_label(Role::TextInput, "editable_text")
            .any(|node| node.value().as_deref() == Some("one edited")),
        "the edited text is shown in the table"
    );
    harness.snapshot("forced_component_types_table_edited");

    // Cards show the first flag in their header and the other flags as labeled fields,
    // which are editable like the table cells.
    harness.get_by_label("Cards view").click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("forced_component_types_cards");

    let card_flags = harness
        .query_all_by_role_and_label(Role::CheckBox, "Flag")
        .collect::<Vec<_>>();
    assert_eq!(card_flags.len(), 9, "cards show every flag field");
    let editable_flags = card_flags
        .iter()
        .filter(|flag| !flag.accesskit_node().is_disabled())
        .collect::<Vec<_>>();
    assert_eq!(
        editable_flags.len(),
        6,
        "the editable flag fields are editable in every card"
    );
    editable_flags[0].click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("forced_component_types_cards_edited");
}

/// A column with `rerun:is_editable` schema metadata is editable without a table blueprint.
#[test]
fn test_schema_editable_column() {
    // Editable cells require a multi-threaded Tokio runtime for blocking credential access.
    let runtime = tokio::runtime::Builder::new_multi_thread() // NOLINT: owned by this test
        .enable_all()
        .build()
        .expect("test runtime should build");
    runtime.block_on(run_schema_editable_column_test());
}

async fn run_schema_editable_column_test() {
    let metadata = |key: &str| [(key.to_owned(), "true".to_owned())].into();
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false)
                .with_metadata(metadata(re_sorbet::metadata::SORBET_IS_TABLE_INDEX)),
            Field::new("notes", DataType::Utf8, false)
                .with_metadata(metadata(re_sorbet::metadata::SORBET_IS_EDITABLE)),
        ],
        Default::default(),
    ));
    let (session_context, table_ref) = common::register_test_table(
        "schema_editable_table",
        schema,
        vec![
            Arc::new(arrow::array::Int64Array::from(vec![1, 2])),
            Arc::new(StringArray::from(vec!["one", "two"])),
        ],
    );

    let mut test_context = TestContext::new();
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
        .expect("test should run inside its Tokio runtime");
    let table_blueprints = TableBlueprints::default();
    let remote_uri: re_uri::EntryUri =
        "rerun+http://localhost:1234/entry/00000000000000000000000000000001"
            .parse()
            .expect("test entry URI should be valid");

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([600.0, 300.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::from(remote_uri.clone()),
                )
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

    assert_eq!(
        harness
            .query_all_by_role_and_label(Role::TextInput, "notes")
            .count(),
        2,
        "each row of the column has a text input"
    );
}

#[expect(clippy::needless_pass_by_value)]
fn setup_component_type_blueprint(
    test_context: &TestContext,
    table_ref: TableReference,
) -> TableBlueprints {
    let blueprint_id = StoreId::random(StoreKind::Blueprint, "component-type-test");
    let mut store_hub = test_context.store_hub.lock();
    let store = store_hub.store_bundle_mut().blueprint_entry(&blueprint_id);
    let timepoint = blueprint_timepoint_for_writes(store);
    let field_order = [
        "name",
        "regular_bool",
        "editable_bool",
        "editable_text",
        "editable_scalar",
        "editable_flag",
        "readonly_flag",
        "second_flag",
        "thumbnail",
        "link",
        "editable_link",
        "entry_kind",
        "unknown_type",
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
        ("name", "Name", None, false),
        ("regular_bool", "Boolean", None, false),
        ("editable_bool", "Editable boolean", None, true),
        ("editable_flag", "Editable flag", Some(Flag::name()), true),
        ("readonly_flag", "Read-only flag", Some(Flag::name()), false),
        ("second_flag", "Second flag", Some(Flag::name()), true),
        ("thumbnail", "Thumbnail", Some(Thumbnail::name()), false),
        ("link", "Link", Some(Url::name()), false),
        ("editable_link", "Editable link", Some(Url::name()), true),
        ("entry_kind", "Entry kind", Some(EntryKind::name()), false),
        (
            "unknown_type",
            "Unknown type",
            Some(ComponentType::from_static_str("example.components.Unknown")),
            false,
        ),
        ("editable_text", "Editable text", None, true),
        ("editable_scalar", "Editable scalar", None, true),
    ];
    for layout in ["table/layouts/table/columns", "table/layouts/cards/fields"] {
        for &(source, name, component_type, editable) in &columns {
            let mut column = TableColumn::new().with_name(name).with_editable(editable);
            if let Some(component_type) = component_type {
                column = column.with_column_type(component_type);
            }
            let chunk = Arc::new(
                Chunk::builder(format!("{layout}/{source}"))
                    .with_archetype_auto_row(timepoint.clone(), &column)
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

fn setup_component_type_table() -> (Arc<SessionContext>, &'static str) {
    let image_path = re_test_context::asset_path("image/grinda.jpg");
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
            Field::new("editable_link", DataType::Utf8, false),
            Field::new("entry_kind", DataType::UInt8, false),
            Field::new("unknown_type", DataType::Utf8, false),
            Field::new("editable_text", DataType::Utf8, false),
            Field::new("editable_scalar", DataType::Float64, false),
        ],
        Default::default(),
    ));
    common::register_test_table(
        "component_type_table",
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
            Arc::new(StringArray::from(LINKS.to_vec())),
            Arc::new(StringArray::from(EDITABLE_LINKS.to_vec())),
            Arc::new(UInt8Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(UNKNOWN_TYPE_VALUES.to_vec())),
            Arc::new(StringArray::from(vec!["one", "two", "three"])),
            Arc::new(Float64Array::from(vec![0.5, 1.5, 2.5])),
        ],
    )
}
