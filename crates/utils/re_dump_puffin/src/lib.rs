//! Converts [puffin](https://github.com/EmbarkStudios/puffin) profiler recordings
//! (`.puffin` files) into JSON, so the scope tree can be analyzed with tools like `jq`.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
    sync::Arc,
};

use anyhow::{Context as _, Result};
use puffin::{FrameData, Reader, Scope, ScopeCollection, ScopeDetails, ScopeId, Stream};
use serde::Serialize;

#[derive(Serialize)]
struct Out<'a> {
    file: &'a str,
    scopes: BTreeMap<u32, ScopeOut>,
    frames: Vec<FrameOut>,
}

#[derive(Serialize)]
struct ScopeOut {
    name: String,
    function: String,
    file: String,
    line: u32,
    kind: &'static str,
}

#[derive(Serialize)]
struct FrameOut {
    frame_index: u64,
    range_ns: (i64, i64),
    duration_ns: i64,
    num_scopes: usize,
    threads: Vec<ThreadOut>,
}

#[derive(Serialize)]
struct ThreadOut {
    name: String,
    start_time_ns: Option<i64>,
    num_scopes: usize,
    depth: usize,
    range_ns: (i64, i64),
    scopes: Vec<ScopeNode>,
}

#[derive(Serialize)]
struct ScopeNode {
    id: u32,
    name: String,
    start_ns: i64,
    duration_ns: i64,
    data: String,
    children: Vec<Self>,
}

fn scope_kind(d: &ScopeDetails) -> &'static str {
    match d.scope_type() {
        puffin::ScopeType::Function => "function",
        puffin::ScopeType::Named => "named",
    }
}

fn resolve_name(id: ScopeId, scopes: &BTreeMap<u32, ScopeOut>) -> String {
    scopes
        .get(&id.0.get())
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("scope#{}", id.0.get()))
}

fn walk(stream: &Stream, offset: u64, scopes: &BTreeMap<u32, ScopeOut>) -> Result<Vec<ScopeNode>> {
    let reader = Reader::with_offset(stream, offset)
        .map_err(|err| anyhow::anyhow!("invalid stream offset: {err:?}"))?; // NOLINT: puffin::Error only implements Debug
    let mut out = Vec::new();
    for scope in reader {
        let scope: Scope<'_> =
            scope.map_err(|err| anyhow::anyhow!("stream parse error: {err:?}"))?; // NOLINT: puffin::Error only implements Debug
        let children = walk(stream, scope.child_begin_position, scopes)?;
        out.push(ScopeNode {
            id: scope.id.0.get(),
            name: resolve_name(scope.id, scopes),
            start_ns: scope.record.start_ns,
            duration_ns: scope.record.duration_ns,
            data: scope.record.data.to_owned(),
            children,
        });
    }
    Ok(out)
}

/// Reads the `.puffin` recording at `path` and writes it as one JSON document to `writer`.
///
/// The JSON layout is `{file, scopes: {id: {name, function, file, line, kind}}, frames: […]}`,
/// where each frame holds its thread streams as nested scope trees.
/// The `scopes` registration map may be empty when the capture did not include scope
/// registration; scope names then fall back to `scope#<id>`.
pub fn dump_to_json_writer(path: &Path, writer: impl Write) -> Result<()> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = BufReader::new(file);

    let mut header = [0u8; 4];
    use std::io::Read as _;
    reader.read_exact(&mut header).context("reading header")?;
    anyhow::ensure!(
        &header == b"PUF0",
        "not a .puffin file (missing PUF0 magic)"
    );

    let mut raw_frames: Vec<Arc<puffin::UnpackedFrameData>> = Vec::new();
    let mut collection = ScopeCollection::default();

    while let Some(frame) = FrameData::read_next(&mut reader).context("reading frame")? {
        for d in &frame.scope_delta {
            collection.insert(d.clone());
        }
        let unpacked = frame.unpacked().context("unpacking frame")?;
        raw_frames.push(unpacked);
    }

    let mut scopes: BTreeMap<u32, ScopeOut> = BTreeMap::new();
    #[expect(clippy::iter_over_hash_type)] // Inserting into a `BTreeMap`, so order is irrelevant.
    for (id, d) in collection.scopes_by_id() {
        scopes.insert(
            id.0.get(),
            ScopeOut {
                name: d.name().to_string(),
                function: d.function_name.to_string(),
                file: d.file_path.to_string(),
                line: d.line_nr,
                kind: scope_kind(d),
            },
        );
    }

    let mut frames = Vec::with_capacity(raw_frames.len());
    for unpacked in &raw_frames {
        let meta = &unpacked.meta;
        let mut threads = Vec::new();
        for (info, stream_info) in &unpacked.thread_streams {
            let nodes = walk(&stream_info.stream, 0, &scopes)?;
            threads.push(ThreadOut {
                name: info.name.clone(),
                start_time_ns: info.start_time_ns,
                num_scopes: stream_info.num_scopes,
                depth: stream_info.depth,
                range_ns: stream_info.range_ns,
                scopes: nodes,
            });
        }
        frames.push(FrameOut {
            frame_index: meta.frame_index,
            range_ns: meta.range_ns,
            duration_ns: meta.range_ns.1 - meta.range_ns.0,
            num_scopes: meta.num_scopes,
            threads,
        });
    }

    let out = Out {
        file: path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        scopes,
        frames,
    };

    let mut writer = BufWriter::new(writer);
    serde_json::to_writer(&mut writer, &out).context("writing JSON")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::dump_to_json_writer;

    #[test]
    fn dump_roundtripped_capture() {
        // Capture one profiled frame the same way `puffin_viewer` exports do.
        // A test capturing a single frame needs no backpressure.
        #[expect(clippy::disallowed_methods)]
        let (frame_tx, frame_rx) = std::sync::mpsc::channel();
        puffin::set_scopes_on(true);
        let sink_id = puffin::GlobalProfiler::lock().add_sink(Box::new(move |frame| {
            frame_tx.send(frame).ok();
        }));
        {
            puffin::profile_scope!("test_scope", "test data");
        }
        puffin::GlobalProfiler::lock().new_frame();
        puffin::GlobalProfiler::lock().remove_sink(sink_id);
        puffin::set_scopes_on(false);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.puffin");
        {
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(b"PUF0").unwrap();
            for frame in frame_rx.try_iter() {
                frame.write_into(None, &mut file).unwrap();
            }
        }

        let mut json_bytes = Vec::new();
        dump_to_json_writer(&path, &mut json_bytes).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();

        let scope_names: Vec<&str> = json["scopes"]
            .as_object()
            .unwrap()
            .values()
            .map(|scope| scope["name"].as_str().unwrap())
            .collect();
        assert!(
            scope_names.contains(&"test_scope"),
            "expected registered scope names to contain `test_scope`, got: {scope_names:?}"
        );

        let frames = json["frames"].as_array().unwrap();
        assert!(!frames.is_empty());
        let threads = frames[0]["threads"].as_array().unwrap();
        let scopes = threads[0]["scopes"].as_array().unwrap();
        assert_eq!(scopes[0]["name"], "test_scope");
        assert_eq!(scopes[0]["data"], "test data");
    }
}
