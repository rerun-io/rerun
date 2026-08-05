//! Frame-time measurement for the text log view with large recordings.
//!
//! See <https://github.com/rerun-io/rerun/issues/7562>.
//!
//! This is not run on CI (timings are machine-dependent); run it manually with:
//! ```sh
//! cargo test -p re_view_text_log --test perf --release -- --ignored --nocapture
//! ```

use std::time::Instant;

use re_chunk::Chunk;
use re_log_types::{TimeInt, Timeline};
use re_sdk_types::archetypes::TextLog;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_text_log::TextView;
use re_viewer_context::{TimeControlCommand, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

const ROWS_PER_CHUNK: usize = 25_000;

#[test]
#[ignore = "manual benchmark, run with --ignored --nocapture"]
fn frame_time_large_log() {
    // Number of chunks of 25k rows each; override with e.g. `PERF_NUM_CHUNKS=80` for 2M rows.
    let num_chunks: usize = std::env::var("PERF_NUM_CHUNKS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let num_rows = num_chunks * ROWS_PER_CHUNK;

    let mut test_context = TestContext::new_with_view_class::<TextView>();
    let timeline = Timeline::log_tick();

    {
        let start = Instant::now();
        let chunks = (0..num_chunks).map(|chunk_idx| {
            let mut builder = Chunk::builder("logs");
            for row in 0..ROWS_PER_CHUNK {
                let tick =
                    i64::try_from(chunk_idx * ROWS_PER_CHUNK + row).expect("row count fits in i64");
                builder = builder.with_archetype_auto_row(
                    [(timeline, tick)],
                    &TextLog::new(format!("log entry {tick}")),
                );
            }
            builder.build().expect("failed to build chunk")
        });
        test_context.add_chunks(chunks);
        eprintln!("built {num_rows} rows in {:.1?}", start.elapsed());
    }

    test_context.set_active_timeline(*timeline.name());
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTime(
            TimeInt::new_temporal(i64::try_from(num_rows / 2).expect("row count fits in i64"))
                .into(),
        )],
    );
    test_context.handle_system_commands(&egui::Context::default());

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(TextView::identifier()))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui(egui::vec2(1024.0, 768.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    // Warm-up: let the view settle (auto-scroll, initial queries, …).
    harness.run_steps(5);

    const NUM_FRAMES: usize = 20;
    let start = Instant::now();
    for _ in 0..NUM_FRAMES {
        harness.step();
    }
    let elapsed = start.elapsed();

    eprintln!(
        "text log view with {num_rows} rows: {:.2} ms/frame (avg over {NUM_FRAMES} frames)",
        elapsed.as_secs_f64() * 1000.0 / NUM_FRAMES as f64
    );
}
