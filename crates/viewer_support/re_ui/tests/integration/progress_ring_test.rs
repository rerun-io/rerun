use egui::vec2;

/// Empty, partly filled (each quadrant), and full, on a 2x display so the arc ends are visible.
#[test]
fn progress_ring() {
    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, vec2(140.0, 24.0))
            .with_pixels_per_point(2.0)
            .build_ui(|ui| {
                ui.horizontal_centered(|ui| {
                    for fraction in [0.0, 0.1, 0.3, 0.6, 0.9, 1.0] {
                        ui.add(re_ui::ProgressRing::new(fraction));
                    }
                });
            });
    harness.run();
    harness.snapshot("progress_ring");
}
