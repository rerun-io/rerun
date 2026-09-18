use std::path::Path;

use egui_kittest::kittest::Queryable as _;
use re_chunk_index::sha256_to_hex;
use re_integration_test::{InspectionHarness, ViewerHarnessExt as _};
use re_log_encoding::{Encoder, EncodingOptions, RrdFingerprint};
use re_log_types::StoreId;
use re_sdk::external::re_log_msg::{LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_sdk::log::{Chunk, ChunkId, RowId};
use re_sdk::time::{TimeCell, TimePoint};
use re_sdk_types::archetypes::Points2D;

const APP_ID: &str = "rerun_example_catalog_test";

#[tokio::test(flavor = "multi_thread")]
async fn viewer_catalog_uses_rrd_fingerprint_layers() {
    let dir = tempfile::tempdir().expect("failed to create catalog test directory");
    let mut recordings = Vec::new();
    for (index, (filename, recording_id, entity)) in [
        ("shared-a.rrd", "shared", "a"),
        ("shared-b.rrd", "shared", "b"),
        ("other.rrd", "other", "c"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = dir.path().join(filename);
        let store_id = StoreId::recording(APP_ID, recording_id);
        let mut encoder = Encoder::new_eager(
            re_build_info::CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            std::fs::File::create(&path).expect("failed to create RRD"),
        )
        .expect("failed to create encoder");
        encoder
            .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
            }))
            .expect("failed to encode store info");

        for (offset, (kind, timepoint)) in [
            ("static", TimePoint::default()),
            (
                "temporal",
                TimePoint::from_iter([("frame", TimeCell::from_sequence(0))]),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let id = (index * 2 + offset + 1) as u128;
            let chunk = Chunk::builder_with_id(ChunkId::from_u128(id), format!("{entity}/{kind}"))
                .with_archetype(
                    RowId::from_u128(id),
                    timepoint,
                    &Points2D::new([(0.0, 0.0), (1.0, 1.0)]),
                )
                .build()
                .expect("failed to build test chunk");
            encoder
                .append(&LogMsg::ArrowMsg(
                    store_id.clone(),
                    chunk.to_arrow_msg().expect("failed to encode chunk"),
                ))
                .expect("failed to append chunk");
        }
        encoder.finish().expect("failed to finish RRD");
        drop(encoder);
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
        open_file(&mut harness, path);
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
        open_file(&mut harness, path);
        harness.step_until("recording entities loaded", |harness| {
            harness.query_by_label("_streams_tree").is_some()
                && harness.query_all_by_label(entity).next().is_some()
                && !harness.is_loading()
        });
    }
}

fn open_file(harness: &mut InspectionHarness, path: &Path) {
    #[cfg(feature = "browser")]
    if InspectionHarness::is_browser() {
        use base64::Engine as _;

        let bytes = std::fs::read(path).expect("failed to read RRD");
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let name = path.file_name().expect("file has a name").to_string_lossy();
        harness.evaluate_js_in_browser(&format!(
            r#"(() => {{
                const data = new DataTransfer();
                data.items.add(new File([Uint8Array.fromBase64({encoded:?})], {name:?}));
                document.querySelector("canvas").dispatchEvent(new DragEvent("drop", {{
                    dataTransfer: data, bubbles: true, cancelable: true,
                }}));
                return "dropped";
            }})()"#
        ));
        return;
    }

    for pressed in [true, false] {
        harness.queue_event(egui::Event::Key {
            key: egui::Key::L,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        });
    }
    harness.run();
    for pressed in [true, false] {
        harness.queue_event(egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
    }
    harness.queue_event(egui::Event::Text(path.display().to_string()));
    harness.run();
    harness.get_by_label("Open").click();
    harness.run_ok();
}
