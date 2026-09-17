//! End-to-end test of the `.puffin` importer: a synthesized capture → `PuffinImporter` → chunks.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use arrow::array::{Array as _, StringArray};
    use puffin::{FrameData, GlobalProfiler, ScopeDetails, Stream, StreamInfo, ThreadInfo};

    use re_chunk::{Chunk, TimelineName};
    use re_importer::{ImportedData, Importer as _, ImporterSettings, PuffinImporter};
    use re_sdk_types::archetypes::StateChange;

    /// A capture of one thread that runs `inner` nested inside `outer`, and `sibling` after both
    /// have ended.
    fn capture_bytes() -> Vec<u8> {
        let ids = GlobalProfiler::lock().register_user_scopes(&[
            ScopeDetails::from_scope_name("outer"),
            ScopeDetails::from_scope_name("inner"),
            ScopeDetails::from_scope_name("sibling"),
        ]);

        let mut stream = Stream::default();
        let outer = stream.begin_scope(|| 100, ids[0], "");
        let inner = stream.begin_scope(|| 150, ids[1], "some data");
        stream.end_scope(inner.0, 250);
        stream.end_scope(outer.0, 400);
        let sibling = stream.begin_scope(|| 500, ids[2], "");
        stream.end_scope(sibling.0, 600);

        let stream_info = StreamInfo::parse(stream).unwrap();
        let thread = ThreadInfo {
            start_time_ns: Some(100),
            name: "worker".to_owned(),
        };

        let frames: Arc<Mutex<Vec<Arc<FrameData>>>> = Arc::default();
        let sink_frames = frames.clone();
        let sink_id = GlobalProfiler::lock().add_sink(Box::new(move |frame| {
            sink_frames.lock().push(frame);
        }));

        GlobalProfiler::lock().report_user_scopes(thread, &stream_info.as_stream_into_ref());
        GlobalProfiler::lock().new_frame();
        GlobalProfiler::lock().remove_sink(sink_id);

        let mut bytes = b"PUF0".to_vec();
        for frame in frames.lock().iter() {
            frame.write_into(None, &mut bytes).unwrap();
        }
        bytes
    }

    fn import_chunks(contents: Vec<u8>) -> Vec<Chunk> {
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = ImporterSettings::recommended("test");
        PuffinImporter
            .import_from_file_contents(
                &settings,
                std::path::PathBuf::from("capture.puffin"),
                contents.into(),
                tx.clone(),
            )
            .unwrap();
        drop(tx);
        rx.iter().filter_map(ImportedData::into_chunk).collect()
    }

    /// The state of every lane, row by row.
    fn lanes_per_row(chunk: &Chunk) -> Vec<Vec<Option<String>>> {
        let column = chunk
            .components()
            .get(StateChange::descriptor_state().component)
            .expect("the importer should have logged state changes");

        (0..column.list_array.len())
            .map(|row| {
                let lanes = column.list_array.value(row);
                let lanes = lanes.as_any().downcast_ref::<StringArray>().unwrap();
                lanes
                    .iter()
                    .map(|lane| lane.map(ToOwned::to_owned))
                    .collect()
            })
            .collect()
    }

    /// A capture with one nested scope and one that follows it turns into a lane per call depth.
    /// The deeper lane is empty while only the outer scope runs, and every lane is empty once the
    /// thread has left all of its scopes.
    #[test]
    fn test_puffin_importer_lanes_follow_the_call_stack() {
        let chunks = import_chunks(capture_bytes());
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];

        assert_eq!(chunk.entity_path().to_string(), "/worker");

        let times = chunk.timelines()[&TimelineName::from("time")].times_raw();
        assert_eq!(times, [100, 150, 250, 400, 500, 600]);

        let outer = Some("outer".to_owned());
        let inner = Some("inner: some data".to_owned());
        let sibling = Some("sibling".to_owned());
        assert_eq!(
            lanes_per_row(chunk),
            vec![
                vec![outer.clone(), None],
                vec![outer.clone(), inner],
                vec![outer, None],
                vec![None, None],
                vec![sibling, None],
                vec![None, None],
            ]
        );

        // The capture holds a single frame, so every row carries the same frame index.
        let frames = chunk.timelines()[&TimelineName::from("frame_nr")].times_raw();
        assert!(frames.iter().all(|frame| *frame == frames[0]));
    }
}
