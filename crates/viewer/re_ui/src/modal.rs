use crate::{DesignTokens, UiExt as _};
use egui::NumExt;

/// Helper object to handle a [`Modal`] window.
///
/// A [`Modal`] is typically held only so long as it is displayed, so it's typically stored in an
/// [`Option`]. This helper object handles that for your.
///
/// Usage:
/// ```
/// # use re_ui::modal::{Modal, ModalHandler};
///
/// # egui::__run_test_ui(|ui| {
/// let mut modal_handler = ModalHandler::default();
///
/// if ui.button("Open").clicked() {
///     modal_handler.open();
/// }
///
/// modal_handler.ui(ui.ctx(), || Modal::new("Modal Window"), |ui, _| {
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
    /// Open the model the next time the [`ModalHandler::ui`] method is called.
    pub fn open(&mut self) {
        self.should_open = true;
    }

    /// Draw the modal window, creating/destroying it as required.
    pub fn ui<R>(
        &mut self,
        ctx: &egui::Context,
        make_modal: impl FnOnce() -> Modal,
        content_ui: impl FnOnce(&mut egui::Ui, &mut bool) -> R,
    ) -> Option<R> {
        if self.modal.is_none() && self.should_open {
            self.modal = Some(make_modal());
            self.should_open = false;
        }

        if let Some(modal) = &mut self.modal {
            let ModalResponse { inner, open } = modal.ui(ctx, content_ui);

            if !open {
                self.modal = None;
            }

            Some(inner)
        } else {
            None
        }
    }
}

/// Response returned by [`Modal::ui`].
//TODO: rename to ModalWrapperResponse
pub struct ModalResponse<R> {
    /// What the content closure returned if it was actually run.
    pub inner: R,

    /// Whether the modal should remain open.
    pub open: bool,
}

/// Show a modal window with Rerun style.
///
/// Relies on [`egui::Modal`].
///
/// TODO:
/// The modal sets the clip rect such as to allow full-span highlighting behavior (e.g. with
/// [`crate::list_item::ListItem`]). Consider using [`crate::UiExt::full_span_separator`] to draw a
/// separator that spans the full width of the modal instead of the usual [`egui::Ui::separator`]
/// method.
///
/// Note that [`Modal`] are typically used via the [`ModalHandler`] helper object to reduce
/// boilerplate.
pub struct Modal {
    title: String,
    min_width: Option<f32>,
    min_height: Option<f32>,
    default_height: Option<f32>,
    full_span_content: bool,
}

//TODO: rename to ModalWrapper
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
    ) -> ModalResponse<R> {
        let id = egui::Id::new(&self.title);

        let mut open = true;

        let mut area = egui::Modal::default_area(id).constrain(true);
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

        ModalResponse {
            inner: modal_response.inner,
            open,
        }

        // Self::dim_background(ctx);
        //
        // // We consume such as to avoid the top-level deselect-on-ESC behavior.
        // let mut open = ctx.input_mut(|i| !i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        //
        // let screen_height = ctx.screen_rect().height();
        // let modal_vertical_margins = (75.0).at_most(screen_height * 0.1);
        //
        //let mut window = egui::Window::new(&self.title)
        //     .pivot(egui::Align2::CENTER_TOP)
        //     .fixed_pos(ctx.screen_rect().center_top() + egui::vec2(0.0, modal_vertical_margins))
        //     .constrain_to(ctx.screen_rect())
        //     .max_height(screen_height - 2.0 * modal_vertical_margins)
        //     .collapsible(false)
        //     .resizable(true)
        //     .frame(egui::Frame {
        //         // Note: inner margin are kept to zero so the clip rect is set to the same size as the modal itself,
        //         // which is needed for the full-span highlighting behavior.
        //         fill: ctx.style().visuals.panel_fill,
        //         ..Default::default()
        //     })
        //     .title_bar(false);
        //
        // if let Some(min_width) = self.min_width {
        //     window = window.min_width(min_width);
        // }
        //
        // if let Some(min_height) = self.min_height {
        //     window = window.min_height(min_height);
        // }
        //
        // if let Some(default_height) = self.default_height {
        //     window = window.default_height(default_height);
        // }
        //
        // let response = window.show(ctx, |ui| {
        //     let item_spacing_y = ui.spacing().item_spacing.y;
        //     ui.spacing_mut().item_spacing.y = 0.0;
        //
        //     egui::Frame {
        //         inner_margin: egui::Margin::symmetric(DesignTokens::view_padding(), 0.0),
        //         ..Default::default()
        //     }
        //     .show(ui, |ui| {
        //         ui.add_space(DesignTokens::view_padding());
        //         Self::title_bar(ui, &self.title, &mut open);
        //         ui.add_space(DesignTokens::view_padding());
        //         ui.full_span_separator();
        //
        //         if self.full_span_content {
        //             // no further spacing for the content UI
        //             content_ui(ui, &mut open)
        //         } else {
        //             // we must restore vertical spacing and add view padding at the bottom
        //             ui.add_space(item_spacing_y);
        //
        //             egui::Frame {
        //                 inner_margin: egui::Margin {
        //                     bottom: DesignTokens::view_padding(),
        //                     ..Default::default()
        //                 },
        //                 ..Default::default()
        //             }
        //             .show(ui, |ui| {
        //                 ui.spacing_mut().item_spacing.y = item_spacing_y;
        //                 content_ui(ui, &mut open)
        //             })
        //             .inner
        //         }
        //     })
        //     .inner
        // });
        //
        // // Any click outside causes the window to close.
        // let cursor_was_over_window = response
        //     .as_ref()
        //     .and_then(|response| {
        //         ctx.input(|i| i.pointer.interact_pos())
        //             .map(|interact_pos| response.response.rect.contains(interact_pos))
        //     })
        //     .unwrap_or(false);
        // if !cursor_was_over_window && ctx.input(|i| i.pointer.any_pressed()) {
        //     open = false;
        // }
    }

    /// Dim the background to indicate that the window is modal.
    #[allow(clippy::needless_pass_by_ref_mut)]
    fn dim_background(ctx: &egui::Context) {
        let painter = egui::Painter::new(
            ctx.clone(),
            egui::LayerId::new(egui::Order::PanelResizeLine, egui::Id::new("DimLayer")),
            egui::Rect::EVERYTHING,
        );
        painter.add(egui::Shape::rect_filled(
            ctx.screen_rect(),
            egui::Rounding::ZERO,
            egui::Color32::from_black_alpha(128),
        ));
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
