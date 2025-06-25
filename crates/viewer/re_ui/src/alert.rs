use crate::design_tokens::AlertVisuals;
use crate::{Icon, UiExt as _, icons};
use egui::{InnerResponse, Response, Ui, Vec2};

enum AlertKind {
    Info,
    Success,
    Warning,
    Error,
}

impl AlertKind {
    fn colors(&self, ui: &Ui) -> &AlertVisuals {
        match self {
            Self::Info => &ui.tokens().info,
            Self::Success => &ui.tokens().success,
            Self::Warning => &ui.tokens().warning,
            Self::Error => &ui.tokens().error,
        }
    }

    fn icon(&self) -> Icon {
        match self {
            Self::Info => icons::INFO,
            Self::Success => icons::SUCCESS,
            Self::Warning => icons::WARNING,
            Self::Error => icons::ERROR,
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
        let colors = self.kind.colors(ui);

        egui::Frame::new()
            .stroke((1.0, colors.stroke))
            .fill(colors.fill)
            .corner_radius(6)
            .inner_margin(6.0)
            .outer_margin(1.0) // Needed because we set clip_rect_margin. TODO(emilk): https://github.com/emilk/egui/issues/4019
    }

    pub fn show<T>(self, ui: &mut Ui, content: impl FnOnce(&mut Ui) -> T) -> InnerResponse<T> {
        self.frame(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                ui.small_icon(&self.kind.icon(), Some(self.kind.colors(ui).icon));
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
