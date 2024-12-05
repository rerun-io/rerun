use crate::{DesignTokens, UiExt as _};

/// Helper object to handle a [`ModalWrapper`] window.
///
/// A [`ModalWrapper`] is typically held only so long as it is displayed, so it's typically stored in an
/// [`Option`]. This helper object handles that for your.
///
/// Usage:
/// ```
/// # use re_ui::modal::{ModalWrapper, ModalHandler};
///
/// # egui::__run_test_ui(|ui| {
/// let mut modal_handler = ModalHandler::default();
///
/// if ui.button("Open").clicked() {
///     modal_handler.open();
/// }
///
/// modal_handler.ui(ui.ctx(), || ModalWrapper::new("Modal Window"), |ui, _| {
///     ui.label("Modal content");
/// });
/// # });
/// ```
#[derive(Default)]
pub struct ModalHandler {
    modal: Option<ModalWrapper>,
    should_open: bool,
}

impl ModalHandler {
    /// Open the model the next time the [`ModalHandler::ui`] method is called.
    pub fn open(&mut self) {
        self.should_open = true;
    }

    /// Draw the modal window, creating/destroying it as required.
    pub fn ui<R>(
        &mut self,
        ctx: &egui::Context,
        make_modal: impl FnOnce() -> ModalWrapper,
        content_ui: impl FnOnce(&mut egui::Ui, &mut bool) -> R,
    ) -> Option<R> {
        if self.modal.is_none() && self.should_open {
            self.modal = Some(make_modal());
            self.should_open = false;
        }

        if let Some(modal) = &mut self.modal {
            let ModalWrapperResponse { inner, open } = modal.ui(ctx, content_ui);

            if !open {
                self.modal = None;
            }

            Some(inner)
        } else {
            None
        }
    }
}

/// Response returned by [`ModalWrapper::ui`].
pub struct ModalWrapperResponse<R> {
    /// What the content closure returned if it was actually run.
    pub inner: R,

    /// Whether the modal should remain open.
    pub open: bool,
}

/// Show a modal window with Rerun style using [`egui::Modal`].
///
/// The modal sets the clip rect such as to allow full-span highlighting behavior (e.g. with
/// [`crate::list_item::ListItem`]). Consider using [`crate::UiExt::full_span_separator`] to draw a
/// separator that spans the full width of the modal instead of the usual [`egui::Ui::separator`]
/// method.
///
/// Note that [`ModalWrapper`] are typically used via the [`ModalHandler`] helper object to reduce
/// boilerplate.
pub struct ModalWrapper {
    title: String,
    min_width: Option<f32>,
    min_height: Option<f32>,
    default_height: Option<f32>,
    full_span_content: bool,
}

impl ModalWrapper {
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
    #[inline]
    pub fn full_span_content(mut self, full_span_content: bool) -> Self {
        self.full_span_content = full_span_content;
        self
    }

    /// Show the modal window.
    ///
    /// Typically called by [`ModalHandler::ui`].
    pub fn ui<R>(
        &self,
        ctx: &egui::Context,
        content_ui: impl FnOnce(&mut egui::Ui, &mut bool) -> R,
    ) -> ModalWrapperResponse<R> {
        let id = egui::Id::new(&self.title);

        let mut open = true;

        let mut area = egui::Modal::default_area(id);
        if let Some(default_height) = self.default_height {
            area = area.default_height(default_height);
        }

        let modal_response = egui::Modal::new("add_view_or_container_modal".into())
            .frame(egui::Frame {
                fill: ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .area(area)
            .show(ctx, |ui| {
                ui.set_clip_rect(ui.max_rect());

                let item_spacing_y = ui.spacing().item_spacing.y;
                ui.spacing_mut().item_spacing.y = 0.0;

                if let Some(min_width) = self.min_width {
                    ui.set_min_width(min_width);
                }

                if let Some(min_height) = self.min_height {
                    ui.set_min_height(min_height);
                }

                egui::Frame {
                    inner_margin: egui::Margin::symmetric(DesignTokens::view_padding(), 0.0),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.add_space(DesignTokens::view_padding());
                    Self::title_bar(ui, &self.title, &mut open);
                    ui.add_space(DesignTokens::view_padding());
                    ui.full_span_separator();

                    if self.full_span_content {
                        // no further spacing for the content UI
                        content_ui(ui, &mut open)
                    } else {
                        // we must restore vertical spacing and add view padding at the bottom
                        ui.add_space(item_spacing_y);

                        egui::Frame {
                            inner_margin: egui::Margin {
                                bottom: DesignTokens::view_padding(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = item_spacing_y;
                            content_ui(ui, &mut open)
                        })
                        .inner
                    }
                })
                .inner
            });

        if modal_response.should_close() {
            open = false;
        }

        ModalWrapperResponse {
            inner: modal_response.inner,
            open,
        }
    }

    /// Display a title bar in our own style.
    fn title_bar(ui: &mut egui::Ui, title: &str, open: &mut bool) {
        ui.horizontal(|ui| {
            ui.strong(title);

            ui.add_space(16.0);

            let mut ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(ui.max_rect())
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            if ui.small_icon_button(&crate::icons::CLOSE).clicked() {
                *open = false;
            }
        });
    }
}
