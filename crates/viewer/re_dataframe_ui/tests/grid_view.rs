//! Test grid view mode of the `DataFusionTableWidget`.

mod common;

use std::sync::Arc;

use arrow::array::{BooleanArray, Float64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::prelude::SessionContext;
use egui::accesskit::Role;
use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_dataframe_ui::DataFusionTableWidget;
use re_test_context::TestContext;
use re_viewer_context::AsyncRuntimeHandle;

use common::run_async_harness;

/// Basic grid view rendering in dark and light theme.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_grid_view() {
    let (session_context, table_ref) = setup_test_table(false);
    let mut snapshot_results = SnapshotResults::new();

    for (theme, suffix) in [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")] {
        let mut test_context = TestContext::new();
        test_context.app_options.experimental.table_grid_view = true;
        let runtime_handle =
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([800.0, 600.0])
            .with_theme(theme)
            .build_ui(|ui| {
                test_context.run_recording(&ui.ctx().clone(), |ctx| {
                    DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                        .title("Grid view test")
                        .show(ctx, &runtime_handle, ui);
                });
            });

        run_async_harness(&mut harness).await;

        // Switch to grid mode.
        harness.get_by_label("Grid view").click();
        run_async_harness(&mut harness).await;

        harness.snapshot(format!("grid_view_basic_{suffix}"));
        snapshot_results.extend_harness(&mut harness);
    }
}

/// Test that the grid reflows when rendered at different widths.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_grid_view_resize() {
    let (session_context, table_ref) = setup_test_table(false);
    let mut snapshot_results = SnapshotResults::new();

    for (width, suffix) in [(400.0, "narrow"), (1200.0, "wide")] {
        let mut test_context = TestContext::new();
        test_context.app_options.experimental.table_grid_view = true;
        let runtime_handle =
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([width, 600.0])
            .build_ui(|ui| {
                test_context.run_recording(&ui.ctx().clone(), |ctx| {
                    DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                        .title("Grid resize test")
                        .show(ctx, &runtime_handle, ui);
                });
            });

        run_async_harness(&mut harness).await;

        // Switch to grid mode.
        harness.get_by_label("Grid view").click();
        run_async_harness(&mut harness).await;

        harness.snapshot(format!("grid_view_resize_{suffix}"));
        snapshot_results.extend_harness(&mut harness);
    }
}

/// Test flag toggle interaction in dark and light theme (in-memory only).
///
/// Verifies that flag buttons appear and clicking toggles the visual state.
/// Server-side persistence of flag changes is tested in
/// `re_integration_test::tests::grid_view_flagging`.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_grid_view_flagging() {
    let (session_context, table_ref) = setup_test_table(true);
    let mut snapshot_results = SnapshotResults::new();

    // Fake remote URI — flagging requires remote_table + table index to be enabled.
    let remote_uri: re_uri::EntryUri =
        "rerun+http://localhost:1234/entry/00000000000000000000000000000001"
            .parse()
            .unwrap();

    for (theme, suffix) in [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")] {
        let mut test_context = TestContext::new();
        test_context.app_options.experimental.table_grid_view = true;
        let runtime_handle =
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([800.0, 600.0])
            .with_theme(theme)
            .build_ui(|ui| {
                test_context.run_recording(&ui.ctx().clone(), |ctx| {
                    DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                        .title("Flag test")
                        .remote_table(remote_uri.clone())
                        .show(ctx, &runtime_handle, ui);
                });
            });

        run_async_harness(&mut harness).await;
        harness.get_by_label("Grid view").click();
        run_async_harness(&mut harness).await;
        harness.snapshot(format!("grid_view_flagging_{suffix}"));

        // Toggle the first flag.
        harness
            .query_all_by_role_and_label(Role::CheckBox, "Flag")
            .next()
            .expect("Expected at least one flag button.")
            .click();
        run_async_harness(&mut harness).await;
        harness.snapshot(format!("grid_view_flagging_toggled_{suffix}"));

        snapshot_results.extend_harness(&mut harness);
    }
}

/// Test grid view with non-uniform card heights to exercise virtualized layout.
///
/// Creates 30 rows with varying content lengths — some with long multi-word notes
/// that wrap, some with short or missing values — so cards end up at different heights.
/// TODO(RR-4405): This looks bad right now.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_grid_view_non_uniform_cards() {
    let (session_context, table_ref) = setup_non_uniform_table();
    let mut test_context = TestContext::new();
    test_context.app_options.experimental.table_grid_view = true;
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([800.0, 600.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                    .title("Non-uniform cards")
                    .show(ctx, &runtime_handle, ui);
            });
        });

    run_async_harness(&mut harness).await;

    // Switch to grid mode.
    harness.get_by_label("Grid view").click();
    run_async_harness(&mut harness).await;

    harness.snapshot("grid_view_non_uniform_cards");
}

// ---

/// Sets up a test table.
///
/// When `with_flagging` is true, the schema is configured for flagging:
/// - `id` column gets `rerun:is_table_index` metadata (required for upsert)
/// - Schema gets `rerun:flag_column` metadata pointing at the `flagged` column
fn setup_test_table(with_flagging: bool) -> (Arc<SessionContext>, &'static str) {
    let mut id_field = Field::new("id", DataType::Int64, false);
    let mut flagged_field = Field::new("flagged", DataType::Boolean, true);

    if with_flagging {
        id_field = id_field.with_metadata(
            [(
                re_sorbet::metadata::SORBET_IS_TABLE_INDEX.to_owned(),
                "true".to_owned(),
            )]
            .into(),
        );
        flagged_field = flagged_field.with_metadata(
            [(
                re_dataframe_ui::experimental_field_metadata::IS_FLAG_COLUMN.to_owned(),
                "true".to_owned(),
            )]
            .into(),
        );
    }

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            id_field,
            Field::new("score", DataType::Float64, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            flagged_field,
            Field::new("notes", DataType::Utf8, true),
        ],
        Default::default(),
    ));
    let batch = RecordBatch::try_new_with_options(
        schema.clone(),
        vec![
            Arc::new(arrow::array::Int64Array::from(vec![1, 2, 3, 4, 5])),
            Arc::new(Float64Array::from(vec![95.0, 82.5, 91.0, 88.0, 76.5])),
            Arc::new(StringArray::from(vec![
                "robotics", "vision", "robotics", "spatial", "vision",
            ])),
            Arc::new(StringArray::from(vec![
                "Alice", "Bob", "Charlie", "Diana", "Eve",
            ])),
            Arc::new(BooleanArray::from(vec![
                Some(true),
                Some(false),
                Some(false),
                Some(true),
                Some(false),
            ])),
            Arc::new(StringArray::from(vec![
                Some("top performer"),
                None,
                Some("needs review"),
                Some("promoted"),
                None,
            ])),
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

/// Sets up a table with 30 rows of wildly varying content lengths.
///
/// Rows differ in: name length, number of nullable fields that are present,
/// description length (from absent to multi-sentence paragraphs), and tag count.
/// This produces cards with very different heights to stress the virtualized
/// layout's height caching and row assignment.
fn setup_non_uniform_table() -> (Arc<SessionContext>, &'static str) {
    let ids: Vec<i64> = (1..=20).collect();
    let n = ids.len();
    let scores: Vec<f64> = (0..n).map(|i| 50.0 + (i as f64 * 1.7) % 50.0).collect();

    let categories: Vec<&str> = (0..n)
        .map(|i| match i % 5 {
            0 => "robotics",
            1 => "computer-vision",
            2 => "spatial-computing",
            3 => "motion-planning",
            _ => "multi-modal-perception",
        })
        .collect();

    let names: Vec<&str> = [
        "Al",
        "Bob",
        "Charlie Chaplin",
        "Di",
        "Eve",
        "Ferdinand von Zeppelin III",
        "G",
        "Hank",
        "Iris Apfel-Strudel",
        "Jo",
        "Kai",
        "Luna Moonbeam Stargazer the Magnificent",
        "Mo",
        "Nia",
        "Olaf",
        "Pippi Longstocking",
        "Q",
        "Raj",
        "Sue",
        "Tiberius Maximus Aurelius",
    ]
    .into();

    // Descriptions vary from None to very long paragraphs.
    let descriptions: Vec<Option<&str>> = (0..n)
        .map(|i| match i % 8 {
            0 => Some(
                "Top performer in the quarterly assessment with outstanding marks across \
                 all evaluation criteria and team collaboration metrics. Recommended for \
                 leadership track. Has consistently demonstrated excellence in cross-functional \
                 projects spanning multiple divisions.",
            ),
            1 | 3 | 5 => None,
            2 => Some("OK"),
            4 => Some(
                "Needs review: flagged by automated pipeline for anomalous sensor readings \
                 during the third calibration pass. Investigate before clearing. The anomaly \
                 pattern matches a known firmware regression in batch 7B units.",
            ),
            6 => Some(
                "Extended field trial participant. Deployed for 847 hours across arctic, \
                 desert, and underwater environments. All subsystems nominal except minor \
                 thermal drift in IMU cluster B which self-corrected after 72h acclimatization. \
                 Full telemetry archive available in dataset DS-2024-0891. Recommend continued \
                 deployment with monthly check-ins.",
            ),
            _ => Some("No issues found."),
        })
        .collect();

    // Tags: some rows have none, some have short tags, some long comma-separated lists.
    let tags: Vec<Option<&str>> = (0..n)
        .map(|i| match i % 6 {
            0 => Some("priority, review-needed, Q4-2024"),
            1 | 4 => None,
            2 => Some("stable"),
            3 => Some("arctic, underwater, extreme-conditions, long-duration, telemetry, thermal-drift, imu"),
            _ => Some("regression, firmware, batch-7B, calibration, sensor-anomaly, high-priority"),
        })
        .collect();

    // Location: mix of present/absent with varying lengths.
    let locations: Vec<Option<&str>> = (0..n)
        .map(|i| match i % 5 {
            0 => Some("Building 4, Lab 2A"),
            1 => Some("Remote — Svalbard Arctic Station, Sector 7G, Cold Storage Unit #12"),
            2 | 4 => None,
            _ => Some("HQ"),
        })
        .collect();

    // Status: short field, always present but varying.
    let statuses: Vec<&str> = (0..n)
        .map(|i| match i % 4 {
            0 => "active",
            1 => "inactive",
            2 => "pending-review",
            _ => "deployed",
        })
        .collect();

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("score", DataType::Float64, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("status", DataType::Utf8, false),
            Field::new("description", DataType::Utf8, true),
            Field::new("tags", DataType::Utf8, true),
            Field::new("location", DataType::Utf8, true),
        ],
        Default::default(),
    ));

    let batch = RecordBatch::try_new_with_options(
        schema,
        vec![
            Arc::new(arrow::array::Int64Array::from(ids)),
            Arc::new(StringArray::from(names)),
            Arc::new(Float64Array::from(scores)),
            Arc::new(StringArray::from(categories)),
            Arc::new(StringArray::from(statuses)),
            Arc::new(StringArray::from(descriptions)),
            Arc::new(StringArray::from(tags)),
            Arc::new(StringArray::from(locations)),
        ],
        &Default::default(),
    )
    .expect("Failed to create a record batch");

    let session_context = Arc::new(SessionContext::new());
    let table_ref = "non_uniform_table";
    session_context
        .register_batch(table_ref, batch)
        .expect("Failed to register the table");

    (session_context, table_ref)
}
