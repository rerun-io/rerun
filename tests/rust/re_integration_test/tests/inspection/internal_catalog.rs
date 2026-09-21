use egui_kittest::kittest::Queryable as _;
use re_chunk_index::sha256_to_hex;
use re_integration_test::{InspectionHarness, ViewerHarnessExt as _};
use re_log_encoding::RrdFingerprint;
use re_sdk::RecordingStreamBuilder;
use re_sdk_types::archetypes::Points2D;

const APP_ID: &str = "rerun_example_catalog_test";

#[tokio::test(flavor = "multi_thread")]
async fn viewer_catalog_uses_rrd_fingerprint_layers() {
    let dir = tempfile::tempdir().expect("failed to create catalog test directory");
    let mut recordings = Vec::new();
    for (filename, recording_id, entity) in [
        ("shared-a.rrd", "shared", "a"),
        ("shared-b.rrd", "shared", "b"),
        ("other.rrd", "other", "c"),
    ] {
        let path = dir.path().join(filename);
        let recording = RecordingStreamBuilder::new(APP_ID)
            .recording_id(recording_id)
            .send_properties(false)
            .save(&path)
            .expect("failed to create .rrd recording stream");
        recording
            .log(entity, &Points2D::new([(0.0, 0.0), (1.0, 1.0)]))
            .expect("failed to log points");
        recording
            .flush_blocking()
            .expect("failed to flush .rrd recording stream");
        drop(recording);
        let fingerprint = RrdFingerprint::compute_for_rrd(
            &std::fs::File::open(&path).expect("failed to open RRD"),
        )
        .await
        .expect("failed to fingerprint RRD");
        recordings.push((
            path,
            format!("rrd-{}", sha256_to_hex(fingerprint.as_bytes())),
            entity,
        ));
    }
    assert_ne!(recordings[0].1, recordings[1].1);

    let mut harness = InspectionHarness::spawn(Default::default());
    for (path, _, _) in &recordings {
        harness.open_file(path);
        harness.step_until("recording loaded", |harness| {
            harness.query_by_label("_streams_tree").is_some() && !harness.is_loading()
        });
    }

    harness.recording_panel().get_label(APP_ID).click();
    harness.step_until("segment table loaded", |harness| {
        harness.query_by_label("Columns").is_some() && !harness.is_loading()
    });
    harness.click_label("Columns");
    harness.click_label("Show column layer names");
    harness.step_until("layer names visible", |harness| {
        harness.query_all_by_label_contains("rrd-").count() == 2
    });
    let shared_cell = harness.get_by_label_contains(&recordings[0].1);
    let other_cell = harness.get_by_label_contains(&recordings[2].1);
    let shared_layers = shared_cell.value().expect("layer cell has a value");
    let a = &recordings[0].1;
    let b = &recordings[1].1;
    assert!(
        [format!("[{a:?}, {b:?}]"), format!("[{b:?}, {a:?}]")].contains(&shared_layers),
        "expected exactly the two shared layers, got {shared_layers}"
    );
    assert_eq!(other_cell.value(), Some(format!("{:?}", recordings[2].1)));
    for (segment, cell) in [("shared", &shared_cell), ("other", &other_cell)] {
        assert!(
            harness
                .get_by_label(segment)
                .rect()
                .y_range()
                .contains(cell.rect().center().y),
            "layer cell must be in the {segment} row"
        );
    }

    for (path, _, entity) in &recordings {
        harness.open_file(path);
        harness.step_until("recording entities loaded", |harness| {
            harness.query_by_label("_streams_tree").is_some()
                && harness.query_all_by_label(entity).next().is_some()
                && !harness.is_loading()
        });
    }
}
