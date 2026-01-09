use eframe::emath::{NumExt as _, Vec2};
use egui::{Frame, ModalResponse};

use crate::context_ext::ContextExt as _;
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
/// modal_handler.ui(ui.ctx(), || ModalWrapper::new("Modal Window"), |ui| {
///     ui.label("Modal content");
/// });
/// # });
/// ```
pub struct ModalHandler {
    modal: Option<ModalWrapper>,
    allow_escape: bool,
    should_open: bool,
    should_close: bool,
}

impl Default for ModalHandler {
    fn default() -> Self {
        Self {
            modal: None,
            allow_escape: true,
            should_open: false,
            should_close: false,
        }
    }
}

impl ModalHandler {
    /// Allow the user to close the modal by interacting with the backdrop and/or pressing escape.
    pub fn allow_escape(mut self, v: bool) -> Self {
        self.allow_escape = v;
        self
    }

    /// Open the modal the next time the [`ModalHandler::ui`] method is called.
    pub fn open(&mut self) {
        self.should_open = true;
    }

    /// Close the modal the next time the [`ModalHandler::ui`] method is called.
    pub fn close(&mut self) {
        self.should_close = true;
    }

    /// Draw the modal window, creating/destroying it as required.
    pub fn ui<R>(
        &mut self,
        ctx: &egui::Context,
        make_modal: impl FnOnce() -> ModalWrapper,
        content_ui: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        if self.modal.is_none() && self.should_open {
            self.modal = Some(make_modal());
            self.should_open = false;
        }

        if let Some(modal) = &mut self.modal {
            let response = modal.ui(ctx, content_ui);

            if self.should_close || (self.allow_escape && response.should_close()) {
                self.modal = None;
            }

            Some(response.inner)
        } else {
            None
        }
    }

    /// Whether the modal is currently open.
    pub fn is_open(&self) -> bool {
        self.modal.is_some()
    }
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
    max_width: Option<f32>,
    min_height: Option<f32>,
    default_height: Option<f32>,
    full_span_content: bool,
    set_side_margins: bool,
    scrollable: egui::Vec2b,
}

impl ModalWrapper {
    /// Create a new modal with the given title.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            min_width: None,
            max_width: None,
            min_height: None,
            default_height: None,
            full_span_content: false,
            set_side_margins: true,
            scrollable: false.into(),
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

    /// Set the maximum width of the modal window.
    #[inline]
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
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

    /// Configure whether the side margin should be set.
    ///
    /// In general, the side margin should be set for a better layout. It may be useful to not set
    /// them if the client code wants to setup a custom scroll area, which should be outside of the
    /// side margins.
    #[inline]
    pub fn set_side_margin(mut self, set_side_margins: bool) -> Self {
        self.set_side_margins = set_side_margins;
        self
    }

    /// Enclose the contents in a scroll area.
    #[inline]
    pub fn scrollable(mut self, scrollable: impl Into<egui::Vec2b>) -> Self {
        self.scrollable = scrollable.into();
        self
    }

    /// Show the modal window.
    ///
    /// Typically called by [`ModalHandler::ui`].
    pub fn ui<R>(
        &self,
        ctx: &egui::Context,
        content_ui: impl FnOnce(&mut egui::Ui) -> R,
    ) -> ModalResponse<R> {
        let tokens = ctx.tokens();
        let id = egui::Id::new(&self.title);

        let mut area = egui::Modal::default_area(id);
        if let Some(default_height) = self.default_height {
            area = area.default_height(default_height);
        }

        egui::Modal::new(id.with("modal"))
            .frame(Frame::new())
            .area(area)
            .show(ctx, |ui| {
                prevent_shrinking(ui);
                egui::Frame {
                    fill: ctx.style().visuals.panel_fill,
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.set_clip_rect(ui.max_rect());

                    let item_spacing_y = ui.spacing().item_spacing.y;
                    ui.spacing_mut().item_spacing.y = 0.0;

                    if let Some(min_width) = self.min_width {
                        ui.set_min_width(min_width);
                    }
                    if let Some(max_width) = self.max_width {
                        ui.set_max_width(max_width);
                    } else {
                        ui.set_max_width(tokens.default_modal_width);
                    }

                    if let Some(min_height) = self.min_height {
                        ui.set_min_height(min_height);
                    }

                    //
                    // Title bar
                    //

                    view_padding_frame(
                        tokens,
                        &ViewPaddingFrameParams {
                            left_and_right: true,
                            top: true,
                            bottom: false,
                        },
                    )
                    .show(ui, |ui| {
                        Self::title_bar(ui, &self.title);
                        ui.add_space(tokens.view_padding() as f32);
                        ui.full_span_separator();
                    });

                    //
                    // Inner content
                    //

                    let wrapped_content_ui = |ui: &mut egui::Ui| -> R {
                        // We always have side margin, but these must happen _inside_ the scroll area
                        // (if any). Otherwise, the scroll bar is not snug with the right border and
                        // may interfere with the action buttons of `ListItem`s.
                        view_padding_frame(
                            tokens,
                            &ViewPaddingFrameParams {
                                left_and_right: self.set_side_margins,
                                top: false,
                                bottom: false,
                            },
                        )
                        .show(ui, |ui| {
                            if self.full_span_content {
                                // no further spacing for the content UI
                                content_ui(ui)
                            } else {
                                // we must restore vertical spacing and add view padding at the bottom
                                ui.add_space(item_spacing_y);

                                view_padding_frame(
                                    tokens,
                                    &ViewPaddingFrameParams {
                                        left_and_right: false,
                                        top: false,
                                        bottom: true,
                                    },
                                )
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = item_spacing_y;
                                    content_ui(ui)
                                })
                                .inner
                            }
                        })
                        .inner
                    };

                    //
                    // Optional scroll area
                    //

                    if self.scrollable.any() {
                        // Make the modal size less jumpy and work around https://github.com/emilk/egui/issues/5138
                        let max_height = 0.85 * ui.ctx().content_rect().height();
                        let min_height = 0.3 * ui.ctx().content_rect().height().at_most(max_height);

                        egui::ScrollArea::new(self.scrollable)
                            .min_scrolled_height(max_height)
                            .max_height(max_height)
                            .show(ui, |ui| {
                                let res = wrapped_content_ui(ui);

                                if ui.min_rect().height() < min_height {
                                    ui.add_space(min_height - ui.min_rect().height());
                                }

                                res
                            })
                            .inner
                    } else {
                        wrapped_content_ui(ui)
                    }
                })
                .inner
            })
    }

    /// Display a title bar in our own style.
    fn title_bar(ui: &mut egui::Ui, title: &str) {
        ui.horizontal(|ui| {
            ui.strong(title);

            ui.add_space(16.0);

            let mut ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(ui.max_rect())
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            if ui
                .small_icon_button(&crate::icons::CLOSE, "Close")
                .clicked()
            {
                ui.close();
            }
        });
    }
}

struct ViewPaddingFrameParams {
    left_and_right: bool,
    top: bool,
    bottom: bool,
}

/// Utility to produce a [`egui::Frame`] with padding on some sides.
#[inline]
fn view_padding_frame(tokens: &DesignTokens, params: &ViewPaddingFrameParams) -> egui::Frame {
    let ViewPaddingFrameParams {
        left_and_right,
        top,
        bottom,
    } = *params;
    egui::Frame {
        inner_margin: egui::Margin {
            left: if left_and_right {
                tokens.view_padding()
            } else {
                0
            },
            right: if left_and_right {
                tokens.view_padding()
            } else {
                0
            },
            top: if top { tokens.view_padding() } else { 0 },
            bottom: if bottom { tokens.view_padding() } else { 0 },
        },
        ..Default::default()
    }
}

/// Prevent a UI from shrinking if the content size changes.
///
/// Should be called at the beginning of the [`egui::Ui`], before any other content is added.
/// Will reset if the screen size changes.
pub fn prevent_shrinking(ui: &mut egui::Ui) {
    // The Uis response at this point will conveniently contain last frame's rect
    let last_rect = ui.response().rect;

    #[expect(clippy::useless_let_if_seq)]
    let mut screen_size = ui.ctx().content_rect().size();
    if ui.is_sizing_pass() {
        // On the very first frame, there will be a sizing pass and the max_rect that frame might
        // be bigger than necessary. We don't want to lock to that size, so we need to ignore it.
        // To ignore, we can't just return here, but we need to skip the next frame as well.
        // The easiest way to do this is force a reset next frame, by changing the screen size:
        screen_size = Vec2::ZERO;
    }

    let id = ui.id().with("prevent_shrinking");
    let screen_size_changed = ui.data_mut(|d| {
        let last_screen_size = d.get_temp_mut_or_insert_with(id, || screen_size);
        let changed = *last_screen_size != screen_size;
        *last_screen_size = screen_size;
        changed
    });

    if last_rect.is_positive() && !screen_size_changed {
        let min_size = ui.min_size();
        // Set the min size, respecting the current min size.
        ui.set_min_size(egui::Vec2::max(last_rect.size(), min_size));
    }
}
