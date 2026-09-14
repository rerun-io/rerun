use re_ui::UiExt as _;

pub(crate) fn info_toggle_button_ui(
    ui: &mut egui::Ui,
    label: &str,
    hover: &str,
    selected: &mut bool,
) {
    ui.medium_icon_toggle_button(&re_ui::icons::INFO, label, selected)
        .on_hover_text(hover);
}

pub(crate) fn copy_button_ui(ui: &mut egui::Ui, label: &str, hover: &str) -> bool {
    ui.small_icon_button(&re_ui::icons::COPY, label)
        .on_hover_text(hover)
        .clicked()
}

pub(crate) fn reset_button_ui(ui: &mut egui::Ui, label: &str, hover: &str) -> bool {
    ui.small_icon_button(&re_ui::icons::RESET, label)
        .on_hover_text(hover)
        .clicked()
}

pub(crate) fn close_button_right_ui(ui: &mut egui::Ui, label: &str, hover: &str) -> bool {
    let mut close_clicked = false;

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui
            .small_icon_button(&re_ui::icons::CLOSE, label)
            .on_hover_text(hover)
            .clicked()
        {
            close_clicked = true;
        }
    });

    close_clicked
}
