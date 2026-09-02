//! Test card layout mode of the `DataFusionTableWidget`.

mod common;

use std::sync::Arc;

use arrow::array::{BooleanArray, Float64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::prelude::SessionContext;
use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_async::AsyncRuntimeHandle;
use re_chunk_store::external::re_chunk::Chunk;
use re_dataframe_ui::{DataFusionTableWidget, TableBlueprints};
use re_log_types::{StoreId, StoreKind};
use re_sdk_types::blueprint::archetypes::CardLayout;
use re_test_context::TestContext;
use re_viewer_context::{TableReference, blueprint_timepoint_for_writes};

use common::run_async_harness;

/// Test that the card layout reflows when rendered at different widths.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_cards_view_resize() {
    let (session_context, table_ref) = setup_test_table();
    let mut snapshot_results = SnapshotResults::new();

    for (width, suffix) in [(400.0, "narrow"), (1200.0, "wide")] {
        let test_context = TestContext::new();
        let runtime_handle =
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();
        let table_blueprints = setup_table_blueprint(
            &test_context,
            TableReference::local("test_table"),
            &["id", "score", "name", "flagged", "notes"],
            "category",
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([width, 600.0])
            .build_ui(|ui| {
                test_context.run_recording(&ui.ctx().clone(), |ctx| {
                    DataFusionTableWidget::new(
                        Arc::clone(&session_context),
                        table_ref,
                        TableReference::local("test_table"),
                    )
                    .title("Cards resize test")
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

        // Switch to card layout.
        harness.get_by_label("Cards view").click();
        run_async_harness(&test_context, &mut harness).await;

        harness.snapshot(format!("cards_view_resize_{suffix}"));
        snapshot_results.extend_harness(&mut harness);
    }
}

/// Test card layout with non-uniform card heights to exercise virtualized layout.
///
/// Creates 30 rows with varying content lengths — some with long multi-word notes
/// that wrap, some with short or missing values — so cards end up at different heights.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_cards_view_non_uniform_cards() {
    let (session_context, table_ref) = setup_non_uniform_table();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();
    let table_blueprints = setup_table_blueprint(
        &test_context,
        TableReference::local("test_table"),
        &[
            "id",
            "score",
            "category",
            "status",
            "description",
            "tags",
            "location",
        ],
        "name",
    );

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([800.0, 600.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .title("Non-uniform cards")
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

    // Switch to card layout.
    harness.get_by_label("Cards view").click();
    run_async_harness(&test_context, &mut harness).await;

    harness.snapshot("cards_view_non_uniform_cards");
}

// ---

#[expect(clippy::needless_pass_by_value)]
fn setup_table_blueprint(
    test_context: &TestContext,
    table_ref: TableReference,
    field_order: &[&str],
    title: &str,
) -> TableBlueprints {
    let blueprint_id = StoreId::random(StoreKind::Blueprint, "table-blueprint-test");
    let mut store_hub = test_context.store_hub.lock();
    let stores = store_hub.store_bundle_mut();
    let store = stores.blueprint_entry(&blueprint_id);
    let timepoint = blueprint_timepoint_for_writes(store);

    let card_layout = Arc::new(
        Chunk::builder("table/layouts/cards")
            .with_archetype_auto_row(
                timepoint,
                &CardLayout::new(field_order.iter().copied()).with_title(title),
            )
            .build()
            .expect("card layout chunk should build"),
    );
    store
        .add_chunk(&card_layout)
        .expect("card layout chunk should be added");

    let mut table_blueprints = TableBlueprints::default();
    table_blueprints
        .set_default_blueprint(&table_ref, &blueprint_id, &mut store_hub)
        .expect("default table blueprint should be set");
    table_blueprints
}

/// Sets up the compact table used by card sizing tests.
fn setup_test_table() -> (Arc<SessionContext>, &'static str) {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("score", DataType::Float64, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("flagged", DataType::Boolean, true),
            Field::new("notes", DataType::Utf8, true),
        ],
        Default::default(),
    ));
    common::register_test_table(
        "test_table",
        schema,
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
    )
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

    common::register_test_table(
        "non_uniform_table",
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
    )
}
