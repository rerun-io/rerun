use re_chunk_store::Chunk;
use re_log_types::Timeline;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::components::Color;

use crate::common::{TextLogTestHarness, logs};

mod manifest;
mod resident;
mod shared;

/// Sparse level/color updates before the viewport must remain visible on text rows.
#[test]
fn view_bootstraps_optional_values() {
    let timeline = Timeline::log_tick();
    let red = Color::from_rgb(255, 0, 0);
    let mut builder = Chunk::builder("logs").with_archetype_auto_row(
        [(timeline, 0)],
        &TextLog::update_fields().with_level("WARN").with_color(red),
    );
    for tick in 1..=150 {
        builder = builder
            .with_archetype_auto_row([(timeline, tick)], &TextLog::new(format!("message {tick}")));
    }
    let chunk = builder.build().expect("valid optional-value fixture");
    for manifest in [false, true] {
        let mut test = TextLogTestHarness::loaded(std::slice::from_ref(&chunk), 75, manifest);
        let painted = test.wait_until(|painted| painted.contains("message 75"));
        assert!(test.missing_chunks().is_empty());
        painted.assert_contains("WARN");
        painted.assert_color("message 75", egui::Color32::RED);
        assert!(!painted.contains("message 1"));
        assert!(!painted.contains("message 150"));
    }
}
