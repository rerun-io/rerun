use egui::Vec2;
use re_ui::UiExt as _;
use re_ui::filter_widget::FilterState;

#[test]
pub fn test_filter_widget() {
    let test_code = |ui: &mut egui::Ui| {
        ui.set_width(100.0);
        ui.set_max_width(100.0);

        FilterState::default().section_title_ui(ui, "Small");

        FilterState::default().section_title_ui(ui, "Expanding available width");

        FilterState::default().section_title_ui_with_buttons(ui, "Multiple actions", |ui| {
            ui.small_icon_button(&re_ui::icons::MORE, "More options");
        });

        ui.set_width(600.0);
        ui.set_max_width(600.0);
        FilterState::default().section_title_ui(ui, "Lots of space");
    };

    let mut harness = egui_kittest::Harness::builder()
        .with_context_setup(re_ui::apply_style_and_install_loaders)
        .with_size(Vec2::new(700.0, 190.0))
        .build_ui(|ui| {
            egui::Panel::right("right_panel").show(ui, |ui| {
                test_code(ui);
            });
        });

    harness.run();

    harness.snapshot("filter_widget");
}
