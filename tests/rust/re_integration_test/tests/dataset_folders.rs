use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotResults};
use re_integration_test::{HarnessExt as _, TestServer};
use re_protos::cloud::v1alpha1::ext::TableInsertMode;
use re_sdk::external::re_log_types::EntryId;
use re_viewer::App;
use re_viewer::external::re_viewer_context::{RedapEntryKind, Route};
use re_viewer::viewer_test_utils::{self, AppTestingExt as _, HarnessOptions};

fn assert_route_and_selection(harness: &mut Harness<'static, App>, expected_route: &Route) {
    let actual_route = harness.state_mut().testonly_get_route().clone();
    assert_eq!(&actual_route, expected_route, "unexpected route");

    let actual_selection =
        harness.run_with_viewer_context(|ctx| ctx.selection().single_item().cloned());
    assert_eq!(
        actual_selection,
        expected_route.item(),
        "unexpected selection"
    );
}

#[tokio::test(flavor = "multi_thread")]
pub async fn dataset_folders() {
    // Seed a hierarchy with mixed subfolders and direct datasets at `perception`:
    //   perception.detection.cars         (grand-child via subfolder `detection`)
    //   perception.detection.audit        (grand-child table via subfolder `detection`)
    //   perception.detection.pedestrians  (grand-child via subfolder `detection`)
    //   perception.metrics                (direct table)
    //   perception.tracking               (direct dataset)
    //   perception.summary                (direct dataset)
    let summary_id_str = "587b552b95a5c2f73f37894708825ba9";
    let (server, _) = TestServer::spawn()
        .await
        .with_named_test_data(
            "perception.detection.cars",
            "287b552b95a5c2f73f37894708825ba6",
            "rec_cars",
        )
        .await;
    let (server, _) = server
        .with_named_test_data(
            "perception.detection.pedestrians",
            "387b552b95a5c2f73f37894708825ba7",
            "rec_pedestrians",
        )
        .await;
    let (server, _) = server
        .with_named_test_data(
            "perception.tracking",
            "487b552b95a5c2f73f37894708825ba8",
            "rec_tracking",
        )
        .await;
    let (server, _) = server
        .with_named_test_data("perception.summary", summary_id_str, "rec_summary")
        .await;

    let mut client = server.client().await.expect("Failed to connect");
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ],
        Default::default(),
    ));
    let metrics_table =
        create_table(&mut client, "perception.metrics", &schema, "metrics-row").await;
    let _audit_table = create_table(
        &mut client,
        "perception.detection.audit",
        &schema,
        "audit-row",
    )
    .await;

    let origin = re_uri::Origin {
        host: re_uri::external::url::Host::Domain("localhost".to_owned()),
        port: server.port(),
        scheme: re_uri::Scheme::RerunHttp,
    };
    let folder_route = |path: &str| Route::RedapEntry {
        origin: origin.clone(),
        kind: RedapEntryKind::Folder(path.to_owned()),
    };
    let summary_route = Route::from(re_uri::EntryUri::new(
        origin.clone(),
        EntryId::from_str(summary_id_str).expect("valid entry id"),
    ));
    let metrics_route = Route::from(re_uri::EntryUri::new(
        origin.clone(),
        metrics_table.details.id,
    ));

    // Jump directly into the `perception` folder via URL.
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/folder/perception",
            server.port()
        )),
        ..Default::default()
    });

    let mut snapshot_results = SnapshotResults::new();

    // Mixed folder: one subfolder card (`detection`) plus direct dataset/table cards.
    viewer_test_utils::step_until(
        "folder `perception` cards appear",
        &mut harness,
        |harness| {
            harness.query_all_by_label_contains("detection").count() == 2
                && harness.query_all_by_label_contains("tracking").count() == 2
                && harness.query_all_by_label_contains("metrics").count() == 2
                && harness.query_all_by_label_contains("summary").count() == 2
                && harness.query_by_label("Loading entries…").is_none()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    assert_route_and_selection(&mut harness, &folder_route("perception"));
    snapshot_results.add(harness.try_snapshot("dataset_folders_01_perception"));

    // Click the subfolder card.
    harness
        .query_all_by_role_and_label(Role::Button, "detection")
        .last()
        .expect("detection folder card should be present")
        .click();
    viewer_test_utils::step_until(
        "folder `perception.detection` cards appear",
        &mut harness,
        |harness| {
            harness
                .query_by_role_and_label(Role::Button, "cars")
                .is_some()
                && harness.query_all_by_label_contains("audit").count() >= 1
                && harness
                    .query_by_role_and_label(Role::Button, "pedestrians")
                    .is_some()
                && harness.query_by_label("Loading entries…").is_none()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    assert_route_and_selection(&mut harness, &folder_route("perception.detection"));
    snapshot_results.add(harness.try_snapshot("dataset_folders_02_perception_detection"));

    // Navigate back up with the parent-folder button.
    harness.get_by_label("Go to parent folder").click();
    viewer_test_utils::step_until(
        "folder `perception` cards reappear after parent navigation",
        &mut harness,
        |harness| {
            harness.query_all_by_label_contains("detection").count() == 2
                && harness.query_all_by_label_contains("tracking").count() == 2
                && harness.query_all_by_label_contains("metrics").count() == 2
                && harness.query_all_by_label_contains("summary").count() == 2
                && harness.query_by_label("Loading entries…").is_none()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    assert_route_and_selection(&mut harness, &folder_route("perception"));
    snapshot_results.add(harness.try_snapshot("dataset_folders_03_perception_after_parent"));

    let mut table_harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/folder/perception",
            server.port()
        )),
        ..Default::default()
    });
    viewer_test_utils::step_until(
        "folder `perception` cards appear for table navigation",
        &mut table_harness,
        |harness| {
            harness.query_all_by_label_contains("detection").count() == 2
                && harness.query_all_by_label_contains("tracking").count() == 2
                && harness.query_all_by_label_contains("metrics").count() == 2
                && harness.query_all_by_label_contains("summary").count() == 2
                && harness.query_by_label("Loading entries…").is_none()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );

    // Click direct table card → navigates to table entry and selects it.
    table_harness
        .query_all_by_role_and_label(Role::Button, "metrics")
        .last()
        .expect("metrics table card should be present")
        .click();
    viewer_test_utils::step_until(
        "table `perception.metrics` row appears",
        &mut table_harness,
        |harness| harness.query_by_label_contains("metrics-row").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    assert_route_and_selection(&mut table_harness, &metrics_route);

    // Click the `summary` dataset card → navigates to the dataset entry and selects it.
    harness
        .query_all_by_role_and_label(Role::Button, "summary")
        .last()
        .expect("summary dataset card should be present")
        .click();
    viewer_test_utils::step_until(
        "dataset `perception.summary` recording appears",
        &mut harness,
        |harness| harness.query_by_label_contains("rec_summary").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    assert_route_and_selection(&mut harness, &summary_route);
    snapshot_results.add(harness.try_snapshot("dataset_folders_04_summary_dataset"));
}

async fn create_table(
    client: &mut re_redap_client::ConnectionClient,
    name: &str,
    schema: &Arc<Schema>,
    row_name: &str,
) -> re_protos::cloud::v1alpha1::ext::TableEntry {
    let batch = RecordBatch::try_new_with_options(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec![row_name, "beta", "gamma"])),
        ],
        &Default::default(),
    )
    .expect("Failed to create record batch");

    let table = client
        .create_table_entry(
            re_sdk::external::re_log_types::EntryName::new(name)
                .expect("Failed to create entry name"),
            None,
            schema.clone(),
        )
        .await
        .expect("Failed to create table");

    client
        .write_table(
            futures::stream::once(async { batch }),
            table.details.id,
            TableInsertMode::Append,
        )
        .await
        .expect("Failed to write table data");

    table
}
