use egui::{Frame, Margin, RichText, Stroke, Style};
use re_ui::{design_tokens, Scale};

pub const CELL_MARGIN: Margin = Margin::symmetric(8, 6);

pub fn apply_table_style_fixes(style: &mut Style) {
    style.visuals.widgets.hovered.bg_stroke =
        Stroke::new(1.0, design_tokens().color_table.gray(Scale::S300));
    style.visuals.widgets.active.bg_stroke =
        Stroke::new(1.0, design_tokens().color_table.gray(Scale::S350));
    style.visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0, design_tokens().color_table.gray(Scale::S200));
}

pub fn header_title(ui: &mut egui::Ui, title: impl Into<RichText>) -> egui::Response {
    header_ui(ui, |ui| {
        ui.monospace(title.into().strong());
    })
    .response
}

pub fn header_ui<R>(
    ui: &mut egui::Ui,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let response = Frame::new()
        .inner_margin(CELL_MARGIN)
        .fill(design_tokens().color_table.gray(Scale::S150))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui)
        });

    let rect = response.response.rect;

    ui.painter().hline(
        rect.x_range(),
        rect.max.y - 1.0, // - 1.0 prevents it from being overdrawn by the following row
        Stroke::new(1.0, design_tokens().color_table.gray(Scale::S300)),
    );

    response
}

pub fn cell_ui<R>(
    ui: &mut egui::Ui,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let response = Frame::new().inner_margin(CELL_MARGIN).show(ui, |ui| {
        ui.set_width(ui.available_width());
        content(ui)
    });

    let rect = response.response.rect;

    ui.painter().hline(
        rect.x_range(),
        rect.max.y - 1.0, // - 1.0 prevents it from being overdrawn by the following row
        Stroke::new(1.0, design_tokens().color_table.gray(Scale::S200)),
    );

    response
}
