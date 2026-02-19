#![cfg(feature = "testing")]

use re_entity_db::EntityDb;
use re_log_types::{StoreId, StoreInfo, StoreKind};
use re_recording_panel::data::RecordingPanelData;
use re_test_context::TestContext;

#[test]
fn empty_context_test() {
    let test_context =
        TestContext::new_with_store_info(StoreInfo::testing_with_recording_id("test_recording"));
    let servers = re_redap_browser::RedapServers::default();

    test_context.run_once_in_egui_central_panel(|ctx, _| {
        let data = RecordingPanelData::new(ctx, &servers, false);

        insta::assert_yaml_snapshot!(data);
    });
}

#[test]
fn fake_local_and_example_recordings_test() {
    let test_context =
        TestContext::new_with_store_info(StoreInfo::testing_with_recording_id("test_recording"));
    let servers = re_redap_browser::RedapServers::default();

    let mut store_hub = test_context.store_hub.lock();

    // fake an example recording
    let mut example_entity_db = EntityDb::new(StoreId::new(
        StoreKind::Recording,
        "rerun_example_dna",
        "dna_rec_id",
    ));
    example_entity_db.data_source = Some(re_log_channel::LogSource::HttpStream {
        url: "https://app.rerun.io/version/nightly/examples/dna.rrd".to_owned(),
        follow: false,
    });
    store_hub.insert_entity_db(example_entity_db);

    // fake a local recording
    let mut local_entity_db = EntityDb::new(StoreId::new(
        StoreKind::Recording,
        "local_app_id",
        "local_rec_id",
    ));
    local_entity_db.data_source = Some(re_log_channel::LogSource::Sdk);
    store_hub.insert_entity_db(local_entity_db);

    // fake a local blueprint (it should not be visible in the recording panel)
    let mut blueprint_entity_db = EntityDb::new(StoreId::new(
        StoreKind::Blueprint,
        "local_app_id",
        "local_blueprint_id",
    ));
    blueprint_entity_db.data_source = Some(re_log_channel::LogSource::Sdk);

    // release the lock lest we deadlock
    drop(store_hub);

    test_context.run_once_in_egui_central_panel(|ctx, _| {
        let data = RecordingPanelData::new(ctx, &servers, false);

        insta::assert_yaml_snapshot!(data);
    });
}
