use crate::ReUi;
use egui::NumExt;

/// Helper object to handle a [`Modal`] window.
///
/// A [`Modal`] is typically held only so long as it is displayed, so it's typically stored in an
/// [`Option`]. This helper object handles that for your.
///
/// Usage:
/// ```
/// # use re_ui::modal::{Modal, ModalHandler};
/// # use re_ui::ReUi;
/// let mut modal_handler = ModalHandler::default();
///
/// # egui::__run_test_ui(|ui| {
/// #   let re_ui = ReUi::load_and_apply(ui.ctx());
/// #   let re_ui = &re_ui;
/// if ui.button("Open").clicked() {
///     modal_handler.open();
/// }
///
/// modal_handler.ui(re_ui, ui, || Modal::new("Modal Window"), |_, ui, _| {
///     ui.label("Modal content");
/// });
/// # });
/// ```
#[derive(Default)]
pub struct ModalHandler {
    modal: Option<Modal>,
    should_open: bool,
}

impl ModalHandler {
    /// Open the model next time the [`ModalHandler::ui`] method is called.
    pub fn open(&mut self) {
        self.should_open = true;
    }

    /// Draw the modal window, creating/destroying it as required.
    pub fn ui<R>(
        &mut self,
        re_ui: &crate::ReUi,
        ui: &mut egui::Ui,
        make_modal: impl FnOnce() -> Modal,
        content_ui: impl FnOnce(&crate::ReUi, &mut egui::Ui, &mut bool) -> R,
    ) -> Option<R> {
        if self.modal.is_none() && self.should_open {
            self.modal = Some(make_modal());
            self.should_open = false;
        }

        if let Some(modal) = &mut self.modal {
            let ModalResponse { inner, open } = modal.ui(re_ui, ui, content_ui);

            if !open {
                self.modal = None;
            }

            inner
        } else {
            None
        }
    }
}

/// Response returned by [`Modal::ui`].
pub struct ModalResponse<R> {
    /// What the content closure returned, if it was actually run.
    pub inner: Option<R>,

    /// Whether the modal should remain open.
    pub open: bool,
}

/// Show a modal window with Rerun style.
///
/// [`Modal`] fakes as a modal window, since egui [doesn't have them yet](https://github.com/emilk/egui/issues/686).
/// This done by dimming the background and capturing clicks outside the window.
///
/// The positioning of the modal is as follows:
///
/// ```text
/// ┌─rerun window─────▲─────────────────────┐
/// │                  │ 75px / 10%          │
/// │          ╔═modal═▼══════════╗  ▲       │
/// │          ║               ▲  ║  │       │
/// │          ║ actual height │  ║  │       │
/// │          ║      based on │  ║  │ max   │
/// │          ║       content │  ║  │ height│
/// │          ║               │  ║  │       │
/// │          ║               ▼  ║  │       │
/// │          ╚══════════════════╝  │       │
/// │          │                  │  │       │
/// │          └───────▲──────────┘  ▼       │
/// │                  │ 75px / 10%          │
/// └──────────────────▼─────────────────────┘
/// ```
///
/// The modal sets the clip rect such as to allow full-span highlighting behavior (e.g. with [`crate::ListItem`]).
/// Consider using [`crate::ReUi::full_span_separator`] to draw a separator that spans the full width of the modal
/// instead of the usual [`egui::Ui::separator`] method.
///
/// Note that [`Modal`] are typically used via the [`ModalHandler`] helper object to reduce boilerplate.
pub struct Modal {
    title: String,
    min_width: Option<f32>,
    min_height: Option<f32>,
    default_height: Option<f32>,
    full_span_content: bool,
}

impl Modal {
    /// Create a new modal with the given title.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            min_width: None,
            min_height: None,
            default_height: None,
            full_span_content: false,
        }
    }

    /// Set the minimum width of the modal window.
    #[inline]
    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width);
        self
    }

    /// Set the minimum height of the modal window.
    #[inline]
    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = Some(min_height);
        self
    }

    /// Set the default height of the modal window.
    #[inline]
    pub fn default_height(mut self, default_height: f32) -> Self {
        self.default_height = Some(default_height);
        self
    }

    /// Configure the content area of the modal for full span highlighting.
    ///
    /// This includes:
    /// - setting the vertical spacing to 0.0
    /// - removing any padding at the bottom of the area
    ///
    /// In this mode, the user code is responsible for adding spacing between items.
    pub fn full_span_content(mut self, full_span_content: bool) -> Self {
        self.full_span_content = full_span_content;
        self
    }

    /// Show the modal window.
    ///
    /// Typically called by [`ModalHandler::ui`].
    pub fn ui<R>(
        &mut self,
        re_ui: &crate::ReUi,
        ui: &mut egui::Ui,
        content_ui: impl FnOnce(&crate::ReUi, &mut egui::Ui, &mut bool) -> R,
    ) -> ModalResponse<R> {
        Self::dim_background(ui);

        let mut open = ui.input(|i| !i.key_pressed(egui::Key::Escape));

        let screen_height = ui.ctx().screen_rect().height();
        let modal_vertical_margins = (75.0).at_most(screen_height * 0.1);

        let mut window = egui::Window::new(&self.title)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(
                ui.ctx().screen_rect().center_top() + egui::vec2(0.0, modal_vertical_margins),
            )
            .constrain_to(ui.ctx().screen_rect())
            .max_height(screen_height - 2.0 * modal_vertical_margins)
            .collapsible(false)
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.visuals().panel_fill,
                //inner_margin: crate::ReUi::view_padding().into(),
                ..Default::default()
            })
            .title_bar(false);

        if let Some(min_width) = self.min_width {
            window = window.min_width(min_width);
        }

        if let Some(min_height) = self.min_height {
            window = window.min_height(min_height);
        }

        if let Some(default_height) = self.default_height {
            window = window.default_height(default_height);
        }

        let response = window.show(ui.ctx(), |ui| {
            let item_spacing_y = ui.spacing().item_spacing.y;
            ui.spacing_mut().item_spacing.y = 0.0;

            egui::Frame {
                inner_margin: egui::Margin::symmetric(ReUi::view_padding(), 0.0),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.add_space(ReUi::view_padding());
                Self::title_bar(re_ui, ui, &self.title, &mut open);
                ui.add_space(ReUi::view_padding());
                crate::ReUi::full_span_separator(ui);

                if self.full_span_content {
                    // no further spacing for the content UI
                    content_ui(re_ui, ui, &mut open)
                } else {
                    // we must restore vertical spacing and add view padding at the bottom
                    ui.add_space(item_spacing_y);

                    egui::Frame {
                        inner_margin: egui::Margin {
                            bottom: ReUi::view_padding(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = item_spacing_y;
                        content_ui(re_ui, ui, &mut open)
                    })
                    .inner
                }
            })
            .inner
        });

        // Any click outside causes the window to close.
        let cursor_was_over_window = response
            .as_ref()
            .and_then(|response| {
                ui.input(|i| i.pointer.interact_pos())
                    .map(|interact_pos| response.response.rect.contains(interact_pos))
            })
            .unwrap_or(false);
        if !cursor_was_over_window && ui.input(|i| i.pointer.any_pressed()) {
            open = false;
        }

        ModalResponse {
            inner: response.and_then(|response| response.inner),
            open,
        }
    }

    /// Dim the background to indicate that the window is modal.
    #[allow(clippy::needless_pass_by_ref_mut)]
    fn dim_background(ui: &mut egui::Ui) {
        let painter = egui::Painter::new(
            ui.ctx().clone(),
            egui::LayerId::new(egui::Order::PanelResizeLine, egui::Id::new("DimLayer")),
            egui::Rect::EVERYTHING,
        );
        painter.add(egui::Shape::rect_filled(
            ui.ctx().screen_rect(),
            egui::Rounding::ZERO,
            egui::Color32::from_black_alpha(128),
        ));
    }

    /// Display a title bar in our own style.
    fn title_bar(re_ui: &crate::ReUi, ui: &mut egui::Ui, title: &str, open: &mut bool) {
        ui.horizontal(|ui| {
            ui.strong(title);

            ui.add_space(16.0);

            let mut ui = ui.child_ui(
                ui.max_rect(),
                egui::Layout::right_to_left(egui::Align::Center),
            );
            if re_ui
                .small_icon_button(&mut ui, &crate::icons::CLOSE)
                .clicked()
            {
                *open = false;
            }
        });
    }
}
