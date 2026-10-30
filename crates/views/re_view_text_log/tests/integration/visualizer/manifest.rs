use re_chunk_store::Chunk;
use re_log_types::{TimePoint, Timeline};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::components::Color;

use super::{TextLogTestHarness, logs};

/// Estimated rows a single navigation span, and therefore one span query, may cover.
#[expect(clippy::cast_possible_wrap)]
const SPAN_ROWS: i64 = (re_chunk_store::ChunkStoreConfig::DEFAULT.chunk_max_rows / 2) as i64;

#[test]
fn manifest_queries_only_the_viewport_and_waits_for_missing_chunks() {
    let timeline = Timeline::log_tick();
    let rows_per_chunk = 100;
    // One span query covers whole chunks up to the span budget, plus the chunk carrying
    // optional components forward into it.
    let max_requested = (SPAN_ROWS / rows_per_chunk + 1) as usize;
    let chunks: Vec<_> = (0..100)
        .map(|chunk| {
            let mut builder = Chunk::builder("logs");
            for row in 0..rows_per_chunk {
                let tick = chunk * rows_per_chunk + row;
                builder = builder.with_archetype_auto_row(
                    [(timeline, tick)],
                    &TextLog::new(format!("Log {tick}")),
                );
            }
            builder.build().unwrap()
        })
        .collect();
    let mut test = TextLogTestHarness::new(Some(&chunks), &[], 5050);
    let (first, missing) = test.frame();
    first.assert_contains("Loading…");
    assert!(!missing.is_empty());
    assert!(
        missing.len() <= max_requested,
        "Requested {} of 100 chunks",
        missing.len()
    );
    assert!(!first.contains("Log 5050"));
    let (waiting, still_missing) = test.frame();
    waiting.assert_contains("Loading…");
    assert!(!still_missing.is_empty());
    let loaded = test.load_until(&chunks, |painted| painted.contains("Log 5050"));
    assert!(!loaded.contains("Log 0"));
    assert!(!loaded.contains("Log 9999"));
    test.context.set_time(9050);
    let (_, missing) = test.frame();
    assert!(!missing.is_empty());
    assert!(missing.len() <= max_requested);
    assert!(missing.contains(&chunks[90].id()));
}

#[test]
fn unrelated_entities_do_not_block_visible_logs() {
    let timeline = Timeline::log_tick();
    let mut builder = Chunk::builder("visible");
    // A full span cannot merge with the other entity's span, and the other entity lies
    // beyond the row overscan.
    for tick in 10..10 + SPAN_ROWS {
        builder = builder.with_archetype_auto_row([(timeline, tick)], &TextLog::new("visible"));
    }
    let visible = builder.build().unwrap();
    let mut chunks = vec![visible.clone()];
    chunks.push(
        Chunk::builder("other")
            .with_archetype_auto_row([(timeline, 10 + SPAN_ROWS + 500)], &TextLog::new("other"))
            .build()
            .unwrap(),
    );
    chunks.push(
        Chunk::builder("other")
            .with_archetype_auto_row(
                [(timeline, 0)],
                &TextLog::update_fields().with_level("WARN"),
            )
            .build()
            .unwrap(),
    );
    let mut test = TextLogTestHarness::new(Some(&chunks), &[visible], 50);
    let (output, missing) = test.frame();
    assert!(
        missing.is_empty(),
        "Unrelated optional chunks were requested: {missing:?}"
    );
    output.assert_contains("visible");
}

#[test]
fn scrolling_during_load_does_not_cancel_cursor_seek() {
    let timeline = Timeline::log_tick();
    let chunk = Chunk::builder("logs")
        .with_archetype_auto_row(
            [(timeline, 0)],
            &TextLog::update_fields().with_many_text(["early"; 1000]),
        )
        .with_archetype_auto_row([(timeline, 100)], &TextLog::new("target"))
        .build()
        .expect("failed to build test chunk");
    let mut test = TextLogTestHarness::new(Some(std::slice::from_ref(&chunk)), &[], 100);
    let (pending, missing) = test.frame();
    pending.assert_contains("Loading…");
    assert_eq!(missing, vec![chunk.id()]);
    test.scroll(-100.0);
    test.context.add_chunks(std::iter::once(chunk));
    test.settle_scroll();
    test.wait_until(|painted| painted.contains("target"));
    // A completed seek must not keep pulling the viewport back after subsequent user input.
    test.scroll(1500.0);
    let scrolled = test.settle_scroll();
    assert!(
        !scrolled.contains("target"),
        "Manual scrolling must persist after refinement"
    );
}

#[test]
fn manifest_static_logs_are_not_requested() {
    let timeline = Timeline::log_tick();
    let static_log = Chunk::builder("static_logs")
        .with_archetype_auto_row(TimePoint::default(), &TextLog::new("static"))
        .build()
        .unwrap();
    let mut chunks = vec![static_log.clone()];
    chunks.extend((0..2).map(|tick| {
        Chunk::builder("temporal_logs")
            .with_archetype_auto_row([(timeline, tick)], &TextLog::new("temporal"))
            .build()
            .unwrap()
    }));
    let mut test = TextLogTestHarness::new(Some(&chunks), &[], 0);
    let (pending, missing) = test.frame();
    pending.assert_contains("Loading…");
    assert!(
        missing.len() <= 2,
        "Expected bounded requests, got {missing:?}"
    );
    assert!(!missing.contains(&static_log.id()));
    test.context.add_chunks(
        chunks
            .iter()
            .filter(|chunk| missing.contains(&chunk.id()))
            .cloned(),
    );
    let loaded = test.wait_until(|painted| painted.contains("temporal"));
    assert!(!loaded.contains("static"));
}

#[test]
fn manifest_bootstraps_optional_components_before_the_requested_span() {
    let timeline = Timeline::log_tick();
    let level = Chunk::builder("logs")
        .with_archetype_auto_row(
            [(timeline, 0)],
            &TextLog::update_fields().with_level("WARN"),
        )
        .build()
        .unwrap();
    let bodies = Chunk::builder("logs")
        .with_archetype_auto_row([(timeline, 100)], &TextLog::new("hello"))
        .build()
        .unwrap();
    let mut test = TextLogTestHarness::new(Some(&[level.clone(), bodies.clone()]), &[bodies], 100);
    let (pending, missing) = test.frame();
    assert!(missing.contains(&level.id()));
    assert!(!pending.contains("hello"));
    test.context.add_chunks(std::iter::once(level));
    let (loaded, _) = test.frame();
    loaded.assert_contains("hello");
    loaded.assert_contains("WARN");
}

#[test]
fn resolved_manifest_rows_are_refetched_after_store_eviction() {
    let chunk = Chunk::builder("logs")
        .with_archetype_auto_row([(Timeline::log_tick(), 10)], &TextLog::new("refetched"))
        .build()
        .unwrap();
    let mut test = TextLogTestHarness::loaded(std::slice::from_ref(&chunk), 10, true);
    let (loaded, _) = test.frame();
    loaded.assert_contains("refetched");
    test.gc_everything();
    let (evicted, missing) = test.frame();
    assert!(
        !evicted.contains("refetched"),
        "Evicted text must not remain on screen"
    );
    evicted.assert_contains("Loading…");
    assert!(
        missing.contains(&chunk.id()),
        "resolved rows must request evicted chunks again"
    );
    test.context.add_chunks(std::iter::once(chunk));
    let (refetched, _) = test.frame();
    refetched.assert_contains("refetched");
}

#[test]
fn available_text_is_displayed_after_optional_payload_eviction() {
    let timeline = Timeline::log_tick();
    let text = Chunk::builder("logs")
        .with_archetype_auto_row([(timeline, 100)], &TextLog::new("available body"))
        .build()
        .unwrap();
    let optional = Chunk::builder("logs")
        .with_archetype_auto_row(
            [(timeline, 0)],
            &TextLog::update_fields()
                .with_level("WARN")
                .with_color(Color::from_rgb(255, 0, 0)),
        )
        .build()
        .unwrap();
    let mut test = TextLogTestHarness::loaded(&[text.clone(), optional.clone()], 100, true);
    test.wait_until(|painted| painted.contains("available body") && painted.contains("WARN"));
    test.gc_everything();
    test.context.add_chunks(std::iter::once(text));
    let (painted, missing) = test.frame();
    painted.assert_contains("available body");
    assert!(missing.contains(&optional.id()));
    assert!(!painted.contains("WARN"));
    test.context.add_chunks(std::iter::once(optional));
    test.wait_until(|painted| painted.contains("available body") && painted.contains("WARN"));
}

#[test]
fn manifest_extension_preserves_resolved_viewport() {
    let original = logs(0..120);
    let mut test = TextLogTestHarness::loaded(std::slice::from_ref(&original), 100, true);
    test.wait_until(|painted| painted.contains("message 100"));
    test.scroll(1000.0);
    let before = test.settle_scroll();
    assert!(!before.contains("message 100"));
    let labels = before.messages();
    assert!(!labels.is_empty());
    let extension = logs(10_000..12_000);
    let manifest = re_chunk_index::RrdManifest::build_in_memory_from_chunks(
        test.context.recording_store_id.clone(),
        std::iter::once(&extension),
    )
    .unwrap();
    test.context.add_rrd_manifest(manifest);
    let (after, missing) = test.frame();
    for label in &labels {
        after.assert_contains(label);
    }
    // Extension fetches may serve overscan, but an unchanged cursor must not center the new tail.
    assert!(!after.contains("message 100"));
    assert!(missing.iter().all(|id| *id == extension.id()));
}
