pub fn mobile_warning_ui(ui: &mut egui::Ui) {
    // On native Android this function is still called but there's nothing to warn about
    // — Android is a supported platform.
    #[cfg(target_os = "android")]
    {
        let _ = ui;
        return;
    }

    #[cfg(not(target_os = "android"))]
    {
        use re_ui::{ContextExt as _, UiExt as _};

        let is_mobile_web = ui.ctx().os() == egui::os::OperatingSystem::IOS
            || ui.ctx().os() == egui::os::OperatingSystem::Android;

        if is_mobile_web {
            let frame = egui::Frame {
                fill: ui.visuals().panel_fill,
                ..ui.tokens().bottom_panel_frame()
            };

            egui::TopBottomPanel::bottom("warning_panel")
                .resizable(false)
                .frame(frame)
                .show_inside(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        let text = ui
                            .ctx()
                            .warning_text("Mobile OSes are not yet supported. Click for details.");
                        ui.hyperlink_to(text, "https://github.com/rerun-io/rerun/issues/1672");
                    });
                });
        }
    }
}
