use crate::{DesignTokens, Icon, UiExt, icons};
use egui::{Color32, InnerResponse, Response, Ui, Vec2};

enum AlertKind {
    Info,
    Success,
    Warning,
    Error,
}

impl AlertKind {
    fn stroke_color(&self, ui: &Ui) -> Color32 {
        match self {
            AlertKind::Info => ui.tokens().border_info,
            AlertKind::Success => ui.tokens().border_success,
            AlertKind::Warning => ui.tokens().border_warning,
            AlertKind::Error => ui.tokens().border_error,
        }
    }

    fn fill_color(&self, ui: &Ui) -> Color32 {
        match self {
            AlertKind::Info => ui.tokens().surface_info,
            AlertKind::Success => ui.tokens().surface_success,
            AlertKind::Warning => ui.tokens().surface_warning,
            AlertKind::Error => ui.tokens().surface_error,
        }
    }

    fn icon_color(&self, ui: &Ui) -> Color32 {
        match self {
            AlertKind::Info => ui.tokens().icon_content_info,
            AlertKind::Success => ui.tokens().icon_content_success,
            AlertKind::Warning => ui.tokens().icon_content_warning,
            AlertKind::Error => ui.tokens().icon_content_error,
        }
    }

    fn icon(&self) -> Icon {
        match self {
            AlertKind::Info => icons::INFO,
            AlertKind::Success => icons::SUCCESS,
            AlertKind::Warning => icons::WARNING,
            AlertKind::Error => icons::ERROR,
        }
    }
}

pub struct Alert {
    kind: AlertKind,
}

impl Alert {
    pub fn success() -> Self {
        Self::new(AlertKind::Success)
    }

    pub fn info() -> Self {
        Self::new(AlertKind::Info)
    }

    pub fn warning() -> Self {
        Self::new(AlertKind::Warning)
    }

    pub fn error() -> Self {
        Self::new(AlertKind::Error)
    }

    fn new(kind: AlertKind) -> Self {
        Self { kind }
    }

    fn frame(&self, ui: &Ui) -> egui::Frame {
        let stroke_color = self.kind.stroke_color(ui);
        let fill_color = self.kind.fill_color(ui);

        egui::Frame::new()
            .stroke((1.0, stroke_color))
            .fill(fill_color)
            .corner_radius(6)
            .inner_margin(6.0)
            .outer_margin(1.0) // Needed because we set clip_rect_margin. TODO(emilk): https://github.com/emilk/egui/issues/4019
    }

    pub fn show<T>(self, ui: &mut Ui, content: impl FnOnce(&mut Ui) -> T) -> InnerResponse<T> {
        self.frame(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                ui.small_icon(&self.kind.icon(), Some(self.kind.icon_color(&ui)));
                content(ui)
            })
            .inner
        })
    }

    pub fn show_text(
        self,
        ui: &mut Ui,
        visible_text: impl Into<String>,
        full_text: Option<String>,
    ) -> Response {
        let visible_text = visible_text.into();
        self.show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            let response = ui.strong(&visible_text).on_hover_ui(|ui| {
                if let Some(full_text) = &full_text {
                    ui.label(full_text);
                    ui.add_space(8.0);
                }
                ui.label("Click to copy text.");
            });
            if response.clicked() {
                ui.ctx().copy_text(full_text.unwrap_or(visible_text));
            };
        })
        .response
    }
}
