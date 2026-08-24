//! Browser tests for exporting recordings and blueprints from the Web Viewer.
//!
//! Each test loads SDK-produced RRD data into the real browser viewer, exports it through the browser API, and verifies that a fresh viewer can ingest the result.
//!
//! Note that these tests call the private `window._handle` methods directly. They do not cover the public `WebViewer` export streams or channel wrappers.

use std::time::Duration;

use base64::Engine as _;
use egui_kittest::kittest::Queryable as _;
use re_entity_db::StoreBundle;
use re_integration_test::{HarnessConfig, InspectionHarness, ViewerHarnessExt as _};
use re_log_channel::LogSource;
use re_log_types::StoreKind;
use re_sdk::RecordingStreamBuilder;
use re_sdk_types::archetypes::Points3D;

/// Num rows produced and expected on read out.
const NUM_ROWS: usize = 10;

#[test]
fn saved_recording_roundtrips_through_a_browser_channel() {
    if !InspectionHarness::is_browser() {
        return;
    }

    let source_bytes = test_recording_bytes();
    let mut source_viewer = InspectionHarness::spawn(HarnessConfig::default());
    send_rrd_to_browser(&source_viewer, &source_bytes, "recording-export-source");
    wait_for_test_recording(&mut source_viewer, "source recording is ingested");

    // Export & validate those bytes.
    let exported_bytes = export_from_browser(&source_viewer, "save_recording");
    assert_exported_recording(&exported_bytes);

    // Ingest them into a separate viewer to make sure it is happy with those.
    let mut destination_viewer = InspectionHarness::spawn(HarnessConfig::default());
    send_rrd_to_browser(
        &destination_viewer,
        &exported_bytes,
        "recording-export-destination",
    );
    wait_for_test_recording(&mut destination_viewer, "exported recording is ingested");
}

#[test]
fn saved_blueprint_roundtrips_through_a_browser_channel() {
    if !InspectionHarness::is_browser() {
        return;
    }

    let recording_and_blueprint_bytes = test_recording_and_blueprint_bytes();
    let mut source_viewer = InspectionHarness::spawn(HarnessConfig::default());
    send_rrd_to_browser(
        &source_viewer,
        &recording_and_blueprint_bytes,
        "blueprint-export-source",
    );
    wait_for_test_recording(&mut source_viewer, "source recording is ingested");
    wait_for_test_blueprint(&mut source_viewer, "source blueprint is ingested");

    // Export blueprint & do basic validation on it.
    let exported_bytes = export_from_browser(&source_viewer, "save_blueprint");
    assert_exported_blueprint(&exported_bytes);

    // Ingest it into a different viewer to make sure it's happy with this blueprint.
    let recording_bytes = test_recording_bytes();
    let mut destination_viewer = InspectionHarness::spawn(HarnessConfig::default());
    send_rrd_to_browser(
        &destination_viewer,
        &recording_bytes,
        "blueprint-export-destination-recording",
    );
    wait_for_test_recording(&mut destination_viewer, "destination recording is ingested");
    send_rrd_to_browser(
        &destination_viewer,
        &exported_bytes,
        "blueprint-export-destination",
    );
    wait_for_test_blueprint(&mut destination_viewer, "exported blueprint is ingested");
}

fn test_recording_bytes() -> Vec<u8> {
    test_recording_bytes_with_blueprint(false)
}

fn test_recording_and_blueprint_bytes() -> Vec<u8> {
    test_recording_bytes_with_blueprint(true)
}

fn test_recording_bytes_with_blueprint(include_blueprint: bool) -> Vec<u8> {
    let recording = RecordingStreamBuilder::new("rerun_example_browser_export_test")
        .recording_id("browser_export_recording")
        .send_properties(false) // Keep the recording small and predictable.
        .buffered()
        .expect("recording stream should start");
    let storage = recording.binary_stream();

    for time in 0..NUM_ROWS {
        recording
            .log("test_entity", &Points3D::new([(time as f32, 0.0, 0.0)]))
            .expect("point should log");
    }

    if include_blueprint {
        re_sdk::blueprint::Blueprint::new(
            re_sdk::blueprint::Spatial3DView::new("Browser export test view")
                .with_contents(["test_entity/**"]),
        )
        .send(
            &recording,
            re_sdk::blueprint::BlueprintActivation::default(),
        )
        .expect("blueprint should send");
    }

    storage
        .flush(Duration::from_secs(5))
        .expect("recording should flush");
    storage.read().expect("recording should contain data")
}

fn send_rrd_to_browser(harness: &InspectionHarness, bytes: &[u8], channel_name: &str) {
    let bytes_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let result = harness.evaluate_js_in_browser(&format!(
        r#"(() => {{
            const id = crypto.randomUUID();
            window._handle.open_channel(id, {channel_name:?});
            window._handle.send_rrd_to_channel(id, Uint8Array.fromBase64({bytes_base64:?}));
            return "sent";
        }})()"#,
    ));
    assert_eq!(result, "sent");
}

fn export_from_browser(harness: &InspectionHarness, method: &str) -> Vec<u8> {
    let exported_base64 = harness.evaluate_js_in_browser(&format!(
        r#"(() => window._handle[{method:?}]().toBase64())()"#,
    ));
    base64::engine::general_purpose::STANDARD
        .decode(exported_base64)
        .expect("browser export should be base64")
}

fn wait_for_test_recording(harness: &mut InspectionHarness, description: &'static str) {
    harness.step_until(description, |harness| {
        harness
            .query_by_label_contains("browser_export_recording")
            .is_some()
            && harness.query_by_label_contains("test_entity").is_some()
    });
}

fn wait_for_test_blueprint(harness: &mut InspectionHarness, description: &'static str) {
    harness.step_until(description, |harness| {
        harness
            .query_all_by_label_contains("Browser export test view")
            .next()
            .is_some()
    });
}

fn assert_exported_blueprint(bytes: &[u8]) {
    let bundle = StoreBundle::from_rrd(
        std::io::BufReader::new(std::io::Cursor::new(bytes)),
        &LogSource::Sdk,
    )
    .expect("exported blueprint should decode");
    let blueprints = bundle
        .entity_dbs()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Blueprint)
        .collect::<Vec<_>>();
    assert_eq!(blueprints.len(), 1);
    assert!(blueprints[0].sorted_entity_paths().any(|path| {
        re_viewer_context::ViewId::from_entity_path(path) != re_viewer_context::ViewId::invalid()
    }));
}

fn assert_exported_recording(bytes: &[u8]) {
    let bundle = StoreBundle::from_rrd(
        std::io::BufReader::new(std::io::Cursor::new(bytes)),
        &LogSource::Sdk,
    )
    .expect("exported recording should decode");
    let recordings = bundle.recordings().collect::<Vec<_>>();
    assert_eq!(recordings.len(), 1);

    let storage_engine = recordings[0].storage_engine();
    let num_rows = storage_engine
        .store()
        .iter_physical_chunks()
        .map(|chunk| chunk.num_rows())
        .sum::<usize>();
    assert_eq!(num_rows, NUM_ROWS);
}
