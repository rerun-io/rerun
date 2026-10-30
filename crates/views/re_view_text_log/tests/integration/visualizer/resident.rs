use re_chunk_store::{Chunk, ChunkStoreConfig};
use re_log_types::Timeline;
use re_sdk_types::archetypes::TextLog;

use super::{TextLogTestHarness, logs};

fn level(time: i64, label: &str) -> Chunk {
    Chunk::builder("logs")
        .with_archetype_auto_row(
            [(Timeline::log_tick(), time)],
            &TextLog::update_fields().with_level(label),
        )
        .build()
        .expect("valid level fixture")
}

#[test]
fn continuous_appends_do_not_starve_the_visible_span() {
    const ROWS: u64 = 6000;
    let required_spans = ROWS.div_ceil(ChunkStoreConfig::DEFAULT.chunk_max_rows / 2);
    assert!(required_spans >= 3);
    let mut test =
        TextLogTestHarness::loaded(&[logs(0..i64::try_from(ROWS).unwrap())], 3000, false);
    // Allow two query/UI feedback round trips per span.
    let bound = 2 * required_spans + 4;
    for frame in 0..bound {
        test.context.add_chunks(std::iter::once(logs([
            10_000 + i64::try_from(frame).unwrap()
        ])));
        test.frame();
    }
    test.frame().0.assert_contains("message 3000");
}

#[test]
fn level_carry_forward_changes_filtered_rows() {
    let mut test = TextLogTestHarness::loaded(&[logs(1..91), level(0, "INFO")], 100, false);
    test.set_level_filter(&["WARN"]);
    test.wait_until(|painted| painted.messages().is_empty() && !painted.contains("Loading…"));
    test.context.add_chunks(std::iter::once(level(0, "WARN")));
    test.wait_until(|painted| painted.contains("message 1"));
    test.context.add_chunks(std::iter::once(level(0, "INFO")));
    test.wait_until(|painted| painted.messages().is_empty() && !painted.contains("Loading…"));
    test.context.add_chunks(std::iter::once(level(0, "WARN")));
    test.wait_until(|painted| painted.contains("message 1"));
}

#[test]
fn resident_gc_removes_evicted_rows_and_later_arrivals_repopulate() {
    // Resident GC keeps the latest chunk, since it cannot be refetched.
    let mut test = TextLogTestHarness::loaded(&[logs(1..50), logs(50..91)], 30, false);
    test.wait_until(|painted| painted.contains("message 30"));
    test.gc_everything();
    let painted =
        test.wait_until(|painted| !painted.messages().is_empty() && !painted.contains("Loading…"));
    assert!(
        painted.messages().iter().all(|message| {
            let time: i64 = message.strip_prefix("message ").unwrap().parse().unwrap();
            (50..91).contains(&time)
        }),
        "Only the retained chunk may remain visible: {painted:?}"
    );
    test.context.set_time(100);
    test.context.add_chunks(std::iter::once(logs([95])));
    test.wait_until(|painted| painted.contains("message 95"));
}
