#![cfg(feature = "testing")]

use re_chunk::{Chunk, RowId, TimePoint};
use re_log_channel::{LogSender, LogSource, RecordingOpenBehavior};
use re_log_msg::{BlueprintActivationCommand, LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_log_types::{ApplicationId, EntryId, StoreId};
use re_sdk_types::archetypes::Points2D;
use re_types_core::SegmentId;
use re_viewer::App;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

fn points_chunk(row_id: RowId) -> Chunk {
    Chunk::builder("foo")
        .with_archetype(row_id, TimePoint::STATIC, &Points2D::new([(0.0, 0.0)]))
        .build()
        .expect("chunk builds")
}

/// Opens a channel as if streaming `recording_segment` of `dataset_id` from a catalog server.
fn add_segment_channel(
    harness: &mut egui_kittest::Harness<'_, App>,
    dataset_id: EntryId,
    recording_segment: &str,
) -> LogSender {
    let (tx, rx) = re_log_channel::log_channel(LogSource::RedapGrpcStream {
        uri: re_uri::DatasetUri {
            origin: re_uri::Origin::http_local_host(re_uri::DEFAULT_PROXY_PORT),
            dataset_id: dataset_id.id,
            resource: re_uri::DatasetResource::Segments,
            segment_id: Some(SegmentId::from(recording_segment)),
            fragment: Default::default(),
        },
        open_behavior: RecordingOpenBehavior::Background,
    });
    harness.state_mut().add_log_receiver(rx);
    tx
}

/// Streams a dataset's default blueprint with one chunk, the way a catalog server does before
/// the recording segment, and returns the row id of that chunk.
fn stream_default_blueprint(tx: &LogSender, blueprint_id: &StoreId) -> RowId {
    send_store_info(tx, blueprint_id);
    let row_id = send_blueprint_chunk(tx, blueprint_id);
    send_make_default(tx, blueprint_id);
    tx.quit(None).expect("receiver is open");

    row_id
}

fn send_store_info(tx: &LogSender, blueprint_id: &StoreId) {
    tx.send(
        LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::new(),
            info: StoreInfo {
                store_id: blueprint_id.clone(),
                store_source: StoreSource::Unknown,
                store_version: None,
            },
        })
        .into(),
    )
    .expect("receiver is open");
}

/// Sends one blueprint chunk and returns its row id.
fn send_blueprint_chunk(tx: &LogSender, blueprint_id: &StoreId) -> RowId {
    let row_id = RowId::new();
    let arrow_msg = points_chunk(row_id).to_arrow_msg().expect("chunk encodes");
    tx.send(LogMsg::ArrowMsg(blueprint_id.clone(), arrow_msg).into())
        .expect("receiver is open");
    row_id
}

fn send_make_default(tx: &LogSender, blueprint_id: &StoreId) {
    tx.send(
        LogMsg::BlueprintActivationCommand(BlueprintActivationCommand {
            blueprint_id: blueprint_id.clone(),
            make_active: false,
            make_default: true,
        })
        .into(),
    )
    .expect("receiver is open");
}

fn latest_row_id(harness: &egui_kittest::Harness<'_, App>, store_id: &StoreId) -> Option<RowId> {
    harness
        .state()
        .store_hub()
        .entity_db(store_id)
        .and_then(|db| db.latest_row_id())
}

/// The dataset's default blueprint streams twice, and the user edits the copy of it in between.
/// The second stream replaces what the store had, and the edited copy stays active.
#[tokio::test]
async fn second_blueprint_stream_replaces_dataset_blueprint() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    let app_id = ApplicationId::from("catalog_blueprint_app");
    let dataset_id = EntryId::new();
    let blueprint_id = re_redap_client::blueprint_store_id_for_segment(
        app_id.clone(),
        &SegmentId::from("default_blueprint"),
    );

    let first_tx = add_segment_channel(&mut harness, dataset_id, "first");
    let first_row_id = stream_default_blueprint(&first_tx, &blueprint_id);
    viewer_test_utils::step_until("first stream is received", &mut harness, |harness| {
        harness
            .state()
            .store_hub()
            .default_blueprint_id_for_app(&app_id)
            == Some(&blueprint_id)
    });
    assert_eq!(latest_row_id(&harness, &blueprint_id), Some(first_row_id));

    let store_hub = harness.state_mut().store_hub_mut();
    store_hub.ensure_active_blueprint_for_app(&app_id);
    let active_id = store_hub
        .active_blueprint_id_for_app(&app_id)
        .expect("active blueprint exists")
        .clone();
    let edited_row_id = RowId::new();
    store_hub
        .add_chunk_for_tests(
            &active_id,
            &std::sync::Arc::new(points_chunk(edited_row_id)),
        )
        .expect("chunk is added");

    let second_tx = add_segment_channel(&mut harness, dataset_id, "second");
    let second_row_id = stream_default_blueprint(&second_tx, &blueprint_id);
    viewer_test_utils::step_until("second stream is received", &mut harness, |harness| {
        latest_row_id(harness, &blueprint_id) == Some(second_row_id)
    });

    let store_hub = harness.state().store_hub();
    assert_eq!(
        store_hub
            .entity_db(&blueprint_id)
            .expect("dataset blueprint exists")
            .num_physical_chunks(),
        1
    );
    assert_eq!(
        store_hub.default_blueprint_id_for_app(&app_id),
        Some(&blueprint_id)
    );
    assert_eq!(
        store_hub.active_blueprint_id_for_app(&app_id),
        Some(&active_id)
    );
    assert_eq!(latest_row_id(&harness, &active_id), Some(edited_row_id));
}

/// Two segments of the same dataset stream its default blueprint at the same time. Once the
/// second stream starts, the rest of the first stream is ignored, and the blueprint store only
/// has the data of the second stream.
#[tokio::test]
async fn newer_blueprint_stream_replaces_older_one() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    let app_id = ApplicationId::from("catalog_blueprint_app");
    let dataset_id = EntryId::new();
    let blueprint_id = re_redap_client::blueprint_store_id_for_segment(
        app_id.clone(),
        &SegmentId::from("default_blueprint"),
    );

    let first_tx = add_segment_channel(&mut harness, dataset_id, "first");
    send_store_info(&first_tx, &blueprint_id);
    let first_row_id = send_blueprint_chunk(&first_tx, &blueprint_id);
    viewer_test_utils::step_until("first stream starts", &mut harness, |harness| {
        latest_row_id(harness, &blueprint_id) == Some(first_row_id)
    });

    let second_tx = add_segment_channel(&mut harness, dataset_id, "second");
    send_store_info(&second_tx, &blueprint_id);
    viewer_test_utils::step_until("second stream starts", &mut harness, |harness| {
        latest_row_id(harness, &blueprint_id).is_none()
    });

    send_blueprint_chunk(&first_tx, &blueprint_id);
    send_make_default(&first_tx, &blueprint_id);
    first_tx.quit(None).expect("receiver is open");
    drop(first_tx);

    let second_row_id = send_blueprint_chunk(&second_tx, &blueprint_id);
    send_make_default(&second_tx, &blueprint_id);
    second_tx.quit(None).expect("receiver is open");
    drop(second_tx);

    viewer_test_utils::step_until("both streams are received", &mut harness, |harness| {
        harness.state().msg_receive_set().is_empty()
    });

    let store_hub = harness.state().store_hub();
    assert_eq!(
        store_hub
            .entity_db(&blueprint_id)
            .expect("dataset blueprint exists")
            .num_physical_chunks(),
        1
    );
    assert_eq!(latest_row_id(&harness, &blueprint_id), Some(second_row_id));
    assert_eq!(
        store_hub.default_blueprint_id_for_app(&app_id),
        Some(&blueprint_id)
    );
}
