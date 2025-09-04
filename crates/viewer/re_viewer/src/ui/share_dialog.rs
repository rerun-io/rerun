use re_log_types::{AbsoluteTimeRange, TimeCell};
use re_redap_browser::EXAMPLES_ORIGIN;

use egui::{AtomExt, NumExt as _};
use re_ui::{UiExt as _, icons, list_item::PropertyContent};
use re_uri::Fragment;
use re_viewer_context::{DisplayMode, Item, RecordingConfig, StoreHub};

use crate::{app::web_viewer_base_url, open_url::ViewerOpenUrl};

pub struct ShareDialog {
    url: Option<ViewerOpenUrl>,
    create_web_viewer_url: bool,
}

#[expect(clippy::derivable_impls)] // False positive.
impl Default for ShareDialog {
    fn default() -> Self {
        Self {
            url: None,
            create_web_viewer_url: cfg!(target_arch = "wasm32"),
        }
    }
}

impl ShareDialog {
    /// Button that opens the share popup.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
        timestamp_format: re_log_types::TimestampFormat,
        active_recording_config: Option<&RecordingConfig>,
        current_selection: Option<&Item>,
    ) {
        re_tracing::profile_function!();

        let popup_id = egui::Id::new("share_dialog_popup");
        let is_panel_visible = egui::Popup::is_id_open(ui.ctx(), popup_id);

        let url_for_current_screen =
            ViewerOpenUrl::from_display_mode(store_hub, display_mode.clone());
        let enable_share_button = !is_panel_visible
            && url_for_current_screen.is_ok()
            && display_mode != &DisplayMode::RedapServer(EXAMPLES_ORIGIN.clone());

        let share_button_resp = ui
            .add_enabled_ui(enable_share_button, |ui| ui.button("Share"))
            .inner;

        let button_response = match url_for_current_screen {
            Err(err) => {
                share_button_resp.on_disabled_hover_text(format!("Cannot create share URL: {err}"))
            }
            Ok(url) => {
                if share_button_resp.clicked() {
                    self.url = Some(url);
                }
                share_button_resp
            }
        };

        if let Some(url) = self.url.as_mut() {
            egui::Popup::from_toggle_button_response(&button_response)
                .id(popup_id)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .frame(ui.tokens().popup_frame(ui.style()))
                .align(egui::RectAlign::BOTTOM_END)
                .show(|ui| {
                    popup_contents(
                        ui,
                        url,
                        &mut self.create_web_viewer_url,
                        timestamp_format,
                        current_selection,
                        active_recording_config,
                    );
                });
        }
    }
}

fn popup_contents(
    ui: &mut egui::Ui,
    url: &mut ViewerOpenUrl,
    create_web_viewer_url: &mut bool,
    timestamp_format: re_log_types::TimestampFormat,
    current_selection: Option<&Item>,
    active_recording_config: Option<&RecordingConfig>,
) {
    let panel_width = 400.0;
    let panel_max_height = (ui.ctx().screen_rect().height() - 100.0)
        .at_least(0.0)
        .at_most(640.0);

    ui.set_width(panel_width);
    ui.set_max_height(panel_max_height);

    ui.horizontal_top(|ui| {
        ui.strong("Share");
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            if ui.small_icon_button(&icons::CLOSE, "Close").clicked() {
                ui.close();
            }
        });
    });

    // Style URL box like a test edit.
    let url_string = {
        let web_viewer_base_url = if *create_web_viewer_url {
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

    let copy_link_response = ui.add(
        egui::Button::new((
            egui::Atom::grow(),
            &icons::INTERNAL_LINK, // TODO: different icon.
            "Copy link",
            egui::Atom::grow(),
        ))
        .fill(ui.tokens().highlight_color)
        .min_size(egui::vec2(panel_width, 20.0)),
    );
    if copy_link_response.clicked() {
        ui.ctx().copy_text(url_string.clone());
        // TODO: feedback. via popup?
    }

    fn style_extra_info(info: String) -> egui::Atom<'static> {
        egui::RichText::new(info).weak().atom_max_width(120.0)
    }

    ui.list_item_scope("url_selection_settings", |ui| {
        ui.list_item_flat_noninteractive(
            PropertyContent::new("Web viewer URL").value_bool_mut(create_web_viewer_url),
        );

        if let Some(url_time_range) = url.time_range_mut() {
            let current_time_range_selection = active_recording_config.and_then(|config| {
                let time_ctrl = config.time_ctrl.read();
                let range = time_ctrl.loop_selection()?;
                Some(re_uri::TimeSelection {
                    timeline: *time_ctrl.timeline(),
                    range: AbsoluteTimeRange::new(range.min.floor(), range.max.ceil()),
                })
            });

            let mut entire_range = url_time_range.is_none();
            ui.list_item_flat_noninteractive(PropertyContent::new("Time range").value_fn(
                |ui, _| {
                    ui.selectable_toggle(|ui| {
                        ui.selectable_value(&mut entire_range, true, "Entire recording");
                        ui.add_enabled_ui(current_time_range_selection.is_some(), |ui| {
                            let mut label = egui::Atoms::new("Selection");
                            if let Some(range) = &current_time_range_selection {
                                let min = TimeCell::new(range.timeline.typ(), range.range.min())
                                    .format_compact(timestamp_format);
                                let max = TimeCell::new(range.timeline.typ(), range.range.max())
                                    .format_compact(timestamp_format);
                                label.push_right(style_extra_info(format!("{min}..{max}")));
                            }

                            ui.selectable_value(&mut entire_range, false, label)
                                .on_disabled_hover_text("No time range selected.");
                        });
                    });
                },
            ));

            if entire_range {
                *url_time_range = None;
            } else {
                *url_time_range = current_time_range_selection;
            }
        }

        if let Some(fragments) = url.fragments_mut() {
            ui.list_item_collapsible_noninteractive_label("Selection", false, |ui| {
                let Fragment { focus, when } = fragments;

                let mut any_focus = focus.is_some();
                let current_selection =
                    current_selection.and_then(|selection| selection.to_data_path());
                ui.list_item_flat_noninteractive(PropertyContent::new("Focus").value_fn(
                    |ui, _| {
                        ui.selectable_toggle(|ui| {
                            ui.selectable_value(&mut any_focus, false, "None");
                            ui.add_enabled_ui(current_selection.is_some(), |ui| {
                                let mut label = egui::Atoms::new("Active");
                                if let Some(current_selection) = &current_selection {
                                    label.push_right(style_extra_info(
                                        current_selection.to_string(),
                                    ));
                                };

                                ui.selectable_value(&mut any_focus, true, label)
                                    .on_disabled_hover_text(
                                        "Current selection can't be embedded in the URL.",
                                    )
                            });
                        });
                    },
                ));
                if any_focus {
                    *focus = current_selection;
                } else {
                    *focus = None;
                }

                let current_time_cursor = active_recording_config.and_then(|config| {
                    let time_ctrl = config.time_ctrl.read();
                    time_ctrl
                        .time_cell()
                        .map(|cell| (*time_ctrl.timeline().name(), cell))
                });
                let mut any_time = when.is_some();
                ui.list_item_flat_noninteractive(PropertyContent::new("Selected time").value_fn(
                    |ui, _| {
                        ui.selectable_toggle(|ui| {
                            ui.selectable_value(&mut any_time, false, "None");
                            ui.add_enabled_ui(current_time_cursor.is_some(), |ui| {
                                let mut label = egui::Atoms::new("Current");
                                if let Some((_, time_cell)) = current_time_cursor {
                                    label.push_right(style_extra_info(
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
    });
}

// TODO: move to re_ui
// struct CopiedFeedbackPopup {
//     start_time: Instant,
// }

// impl CopiedFeedbackPopup {
//     fn new() -> Self {
//         Self {
//             start_time: Instant::now(),
//         }
//     }
// }
