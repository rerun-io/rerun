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
/// Note that [`Modal`] are typically used via the [`ModalHandler`] helper object to reduce boilerplate.
pub struct Modal {
    title: String,
    default_height: Option<f32>,
}

impl Modal {
    /// Create a new modal with the given title.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            default_height: None,
        }
    }

    /// Set the default height of the modal window.
    #[inline]
    pub fn default_height(mut self, default_height: f32) -> Self {
        self.default_height = Some(default_height);
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

        let mut window = egui::Window::new(&self.title)
            .pivot(egui::Align2::CENTER_CENTER)
            .fixed_pos(ui.ctx().screen_rect().center())
            .constrain_to(ui.ctx().screen_rect())
            .collapsible(false)
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.visuals().panel_fill,
                inner_margin: crate::ReUi::view_padding().into(),
                ..Default::default()
            })
            .title_bar(false);

        if let Some(default_height) = self.default_height {
            window = window.default_height(default_height);
        }

        let response = window.show(ui.ctx(), |ui| {
            Self::title_bar(re_ui, ui, &self.title, &mut open);
            content_ui(re_ui, ui, &mut open)
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
        ui.separator();
    }
}
