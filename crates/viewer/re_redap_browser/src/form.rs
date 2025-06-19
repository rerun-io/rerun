use egui::{Align, Grid, Layout, Response, Ui, Vec2, Widget, WidgetText};

pub struct Form {
    name: String,
}

impl Form {
    /// The name should be locally unique
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn show(self, ui: &mut egui::Ui, content: impl FnOnce(&mut Ui)) -> Response {
        Grid::new(&self.name)
            .num_columns(2)
            .spacing(Vec2::new(16.0, 12.0))
            .show(ui, content)
            .response
    }
}

pub struct FormField {
    label: WidgetText,
    hint_text: Option<String>,
    error: bool,
}

impl FormField {
    pub fn new(label: impl Into<WidgetText>) -> Self {
        Self {
            label: label.into(),
            hint_text: None,
            error: false,
        }
    }

    pub fn hint(mut self, hint_text: impl Into<String>) -> Self {
        self.hint_text = Some(hint_text.into());
        self
    }

    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, content: impl Widget) -> Response {
        ui.label(self.label);

        let response = ui
            .with_layout(Layout::top_down_justified(Align::Min), |ui| {
                if self.error {
                    style_invalid_field(ui);
                }

                let response = ui.add(content);
                if let Some(hint) = &self.hint_text {
                    // ui.end_row();
                    // ui.small("");
                    ui.small(hint);
                }
                response
            })
            .inner;

        ui.end_row();

        response
    }
}

fn style_invalid_field(ui: &mut egui::Ui) {
    ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::new(1.0, ui.visuals().error_fg_color);
    ui.visuals_mut().widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, ui.visuals().error_fg_color);
    ui.visuals_mut().widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, ui.visuals().error_fg_color);
}
