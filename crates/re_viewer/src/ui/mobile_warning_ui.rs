pub fn mobile_warning_ui(re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
    // We have not yet optimized the UI experience for mobile. Show a warning banner
    // with a link to the tracking issue.

    if ui.ctx().os() == egui::os::OperatingSystem::IOS
        || ui.ctx().os() == egui::os::OperatingSystem::Android
    {
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..re_ui.bottom_panel_frame()
        };

        egui::TopBottomPanel::bottom("warning_panel")
            .resizable(false)
            .frame(frame)
            .show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let text =
                        re_ui.warning_text("Mobile OSes are not yet supported. Click for details.");
                    ui.hyperlink_to(text, "https://github.com/rerun-io/rerun/issues/1672");
                });
            });
    }
}
