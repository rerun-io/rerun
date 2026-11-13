use egui::vec2;
use re_ui::UiExt as _;

#[test]
fn loading_screen() {
    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, vec2(700.0, 300.0))
            .with_pixels_per_point(2.0)
            .build_ui(|ui| {
                ui.loading_screen(
                    "Loading dataset",
                    "rerun://api.example.com:443/entry/18D527208018DD7D77d2a650391a5383",
                    None,
                    None,
                );
            });
    harness.run_steps(4);
    harness.snapshot("loading_screen");
}
