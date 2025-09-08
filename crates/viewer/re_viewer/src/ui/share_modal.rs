use egui::{AtomExt as _, IntoAtoms as _, NumExt as _};
use web_time::{Duration, Instant};

use re_log_types::{AbsoluteTimeRange, TimeCell};
use re_redap_browser::EXAMPLES_ORIGIN;
use re_ui::{
    HasDesignTokens, UiExt as _, icons,
    list_item::PropertyContent,
    modal::{ModalHandler, ModalWrapper},
};
use re_uri::Fragment;
use re_viewer_context::{DisplayMode, Item, RecordingConfig, StoreHub};

use crate::{app::web_viewer_base_url, open_url::ViewerOpenUrl};

const COPIED_FEEDBACK_DURATION: Duration = Duration::from_millis(500);

pub struct ShareModal {
    modal: ModalHandler,

    url: Option<ViewerOpenUrl>,
    create_web_viewer_url: bool,
    last_time_copied: Option<Instant>,
}

#[expect(clippy::derivable_impls)] // False positive.
impl Default for ShareModal {
    fn default() -> Self {
        Self {
            modal: ModalHandler::default(),

            url: None,
            create_web_viewer_url: cfg!(target_arch = "wasm32"),
            last_time_copied: None,
        }
    }
}

impl ShareModal {
    pub fn open(&mut self, url_for_current_screen: ViewerOpenUrl) {
        self.modal.open();
        self.url = Some(url_for_current_screen);
    }

    /// Button that opens the share popup.
    pub fn button_ui(
        &mut self,
        ui: &mut egui::Ui,
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
    ) {
        re_tracing::profile_function!();

        let url_for_current_screen =
            ViewerOpenUrl::from_display_mode(store_hub, display_mode.clone());
        let enable_share_button = url_for_current_screen.is_ok()
            && display_mode != &DisplayMode::RedapServer(EXAMPLES_ORIGIN.clone());

        let share_button_resp = ui
            .add_enabled_ui(enable_share_button, |ui| ui.button("Share"))
            .inner;

        match url_for_current_screen {
            Err(err) => {
                share_button_resp.on_disabled_hover_text(format!("Cannot create share URL: {err}"));
            }
            Ok(url) => {
                if share_button_resp.clicked() {
                    self.open(url);
                }
            }
        };
    }

    pub fn ui(
        &mut self,
        ui: &egui::Ui,
        timestamp_format: re_log_types::TimestampFormat,
        current_selection: Option<&Item>,
        rec_cfg: &RecordingConfig,
    ) {
        let Some(url) = &mut self.url else {
            // Happens only if the modal is closed anyways.
            debug_assert!(!self.modal.is_open());
            return;
        };

        let panel_width = 500.0;

        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Share").max_width(panel_width),
            |ui| {
                let panel_max_height = (ui.ctx().screen_rect().height() - 100.0)
                    .at_least(0.0)
                    .at_most(640.0);
                ui.set_max_height(panel_max_height);

                // Style URL box like a test edit.
                let url_string = {
                    let web_viewer_base_url = if self.create_web_viewer_url {
                        web_viewer_base_url()
                    } else {
                        None
                    };
                    let mut url_string = url
                        .sharable_url(web_viewer_base_url.as_ref())
                        .unwrap_or_default();

                    egui::TextEdit::singleline(&mut url_string)
                        .hint_text("<can't share link>") // No known way to get into this situation.
                        .text_color(ui.style().visuals.strong_text_color())
                        .interactive(false) // We don't actually want to edit the URL, but text edit is stylewise what we want here.
                        .desired_width(f32::INFINITY)
                        .show(ui);

                    url_string
                };

                let copy_link_label = if self
                    .last_time_copied
                    .is_some_and(|t| t.elapsed() < COPIED_FEEDBACK_DURATION)
                {
                    (
                        egui::Atom::grow(),
                        "Copied to clipboard!",
                        egui::Atom::grow(),
                    )
                        .into_atoms()
                } else {
                    (
                        egui::Atom::grow(),
                        icons::URL.as_image().tint(ui.tokens().icon_inverse),
                        "Copy link",
                        egui::Atom::grow(),
                    )
                        .into_atoms()
                };
                let copy_link_response = ui
                    .scope(|ui| {
                        let tokens = ui.tokens();
                        let visuals = &mut ui.style_mut().visuals;
                        visuals.override_text_color = Some(tokens.text_inverse);

                        let response = ui.ctx().read_response(ui.next_auto_id());
                        let fill_color = if response.is_some_and(|r| r.hovered()) {
                            tokens.bg_fill_inverse_hover
                        } else {
                            tokens.bg_fill_inverse
                        };

                        ui.add(
                            egui::Button::new(copy_link_label)
                                .fill(fill_color)
                                .min_size(egui::vec2(ui.available_width(), 20.0)),
                        )
                    })
                    .inner;
                if copy_link_response.clicked() {
                    ui.ctx().copy_text(url_string.clone());
                    self.last_time_copied = Some(Instant::now());
                }

                ui.list_item_scope("share_dialog_url_settings", |ui| {
                    url_settings_ui(
                        ui,
                        url,
                        &mut self.create_web_viewer_url,
                        timestamp_format,
                        current_selection,
                        rec_cfg,
                    );
                });
            },
        );
    }
}

fn url_settings_ui(
    ui: &mut egui::Ui,
    url: &mut ViewerOpenUrl,
    create_web_viewer_url: &mut bool,
    timestamp_format: re_log_types::TimestampFormat,
    current_selection: Option<&Item>,
    rec_cfg: &RecordingConfig,
) {
    ui.list_item_flat_noninteractive(
        PropertyContent::new("Web viewer URL").value_bool_mut(create_web_viewer_url),
    );

    if let Some(url_time_range) = url.time_range_mut() {
        time_range_ui(ui, url_time_range, timestamp_format, rec_cfg);
    }

    if let Some(fragments) = url.fragments_mut() {
        fragments_ui(ui, fragments, timestamp_format, current_selection, rec_cfg);
    }
}

fn time_range_ui(
    ui: &mut egui::Ui,
    url_time_range: &mut Option<re_uri::TimeSelection>,
    timestamp_format: re_log_types::TimestampFormat,
    rec_cfg: &RecordingConfig,
) {
    let current_time_range_selection = {
        let time_ctrl = rec_cfg.time_ctrl.read();
        time_ctrl
            .loop_selection()
            .map(|range| re_uri::TimeSelection {
                timeline: *time_ctrl.timeline(),
                range: AbsoluteTimeRange::new(range.min.floor(), range.max.ceil()),
            })
    };

    // TODO(#10814): still missing snapshot handling.

    let mut entire_range = url_time_range.is_none();
    ui.list_item_flat_noninteractive(PropertyContent::new("Time range").value_fn(|ui, _| {
        ui.selectable_toggle(|ui| {
            ui.selectable_value(&mut entire_range, true, "Entire recording");
            ui.add_enabled_ui(current_time_range_selection.is_some(), |ui| {
                let mut label = egui::Atoms::new("Selection");
                if let Some(range) = &current_time_range_selection {
                    let min = TimeCell::new(range.timeline.typ(), range.range.min())
                        .format_compact(timestamp_format);
                    let max = TimeCell::new(range.timeline.typ(), range.range.max())
                        .format_compact(timestamp_format);
                    label.push_right(format_extra_toggle_info(format!("{min}..{max}")));
                }

                ui.selectable_value(&mut entire_range, false, label)
                    .on_disabled_hover_text("No time range selected.");
            });
        });
    }));

    if entire_range {
        *url_time_range = None;
    } else {
        *url_time_range = current_time_range_selection;
    }
}

fn fragments_ui(
    ui: &mut egui::Ui,
    fragments: &mut Fragment,
    timestamp_format: re_log_types::TimestampFormat,
    current_selection: Option<&Item>,
    rec_cfg: &RecordingConfig,
) {
    ui.list_item_collapsible_noninteractive_label("Selection", false, |ui| {
        let Fragment { focus, when } = fragments;

        let mut any_focus = focus.is_some();
        let current_selection = current_selection.and_then(|selection| selection.to_data_path());
        ui.list_item_flat_noninteractive(PropertyContent::new("Focus").value_fn(|ui, _| {
            ui.selectable_toggle(|ui| {
                ui.selectable_value(&mut any_focus, false, "None");
                ui.add_enabled_ui(current_selection.is_some(), |ui| {
                    let mut label = egui::Atoms::new("Active");
                    if let Some(current_selection) = &current_selection {
                        label.push_right(format_extra_toggle_info(current_selection.to_string()));
                    }

                    let disabled_reason = if current_selection.is_none() {
                        "No selection."
                    } else {
                        "Current selection can't be embedded in the URL."
                    };
                    ui.selectable_value(&mut any_focus, true, label)
                        .on_disabled_hover_text(disabled_reason)
                });
            });
        }));
        if any_focus {
            *focus = current_selection;
        } else {
            *focus = None;
        }

        let current_time_cursor = {
            let time_ctrl = rec_cfg.time_ctrl.read();
            time_ctrl
                .time_cell()
                .map(|cell| (*time_ctrl.timeline().name(), cell))
        };
        let mut any_time = when.is_some();
        ui.list_item_flat_noninteractive(PropertyContent::new("Selected time").value_fn(
            |ui, _| {
                ui.selectable_toggle(|ui| {
                    ui.selectable_value(&mut any_time, false, "None");
                    ui.add_enabled_ui(current_time_cursor.is_some(), |ui| {
                        let mut label = egui::Atoms::new("Current");
                        if let Some((_, time_cell)) = current_time_cursor {
                            label.push_right(format_extra_toggle_info(
                                time_cell.format_compact(timestamp_format),
                            ));
                        }

                        ui.selectable_value(&mut any_time, true, label)
                            .on_disabled_hover_text("No time selected.");
                    });
                });
            },
        ));
        if any_time {
            *when = current_time_cursor;
        } else {
            *when = None;
        }
    });
}

fn format_extra_toggle_info(info: String) -> egui::Atom<'static> {
    egui::RichText::new(info).weak().atom_max_width(120.0)
}
