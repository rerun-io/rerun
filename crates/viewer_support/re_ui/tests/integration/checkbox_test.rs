use egui::Vec2;
use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_ui::UiExt as _;

#[test]
pub fn test_checkbox_and_radio() {
    let mut results = SnapshotResults::new();
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut harness =
            re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(160.0, 140.0))
                .with_theme(theme)
                .build_ui(|ui| {
                    ui.checkbox(&mut false, "Unchecked");
                    ui.checkbox(&mut true, "Checked");
                    ui.checkbox(&mut false, "Hovered");
                    let mut value = 0;
                    ui.re_radio_value(&mut value, 0, "Selected");
                    ui.re_radio_value(&mut value, 1, "Unselected");
                });

        harness.get_by_label("Hovered").hover();
        harness.run();
        harness.snapshot(format!("checkbox_and_radio_{theme:?}").to_lowercase());
        results.extend_harness(&mut harness);
    }
}
