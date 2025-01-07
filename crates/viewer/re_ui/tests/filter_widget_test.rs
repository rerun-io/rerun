use egui::Vec2;

use re_ui::FilterState;

#[test]
pub fn test_filter_widget() {
    let test_code = |ui: &mut egui::Ui| {
        ui.set_width(100.0);
        ui.set_max_width(100.0);

        FilterState::default().ui(ui, egui::RichText::new("Small").strong());

        FilterState::default().ui(
            ui,
            egui::RichText::new("Expanding available width").strong(),
        );

        ui.set_width(600.0);
        ui.set_max_width(600.0);
        FilterState::default().ui(ui, egui::RichText::new("Lots of space").strong());
    };

    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(700.0, 150.0))
        .build(|ctx| {
            egui::SidePanel::right("right_panel").show(ctx, |ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                test_code(ui);
            });
        });

    harness.run();

    //TODO(#8245): enable this everywhere when we have a software renderer setup
    #[cfg(target_os = "macos")]
    harness.wgpu_snapshot("filter_widget");
}
