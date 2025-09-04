use re_chunk::TimelineName;
use re_log_types::TimeCell;
use re_redap_browser::EXAMPLES_ORIGIN;

use egui::NumExt as _;
use re_ui::{UiExt as _, icons, list_item::PropertyContent};
use re_uri::Fragment;
use re_viewer_context::{DisplayMode, StoreHub};

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
        current_time_selection: re_uri::TimeSelection,
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
                        current_time_selection,
                    );
                });
        }
    }
}

fn popup_contents(
    ui: &mut egui::Ui,
    url: &mut ViewerOpenUrl,
    create_web_viewer_url: &mut bool,
    current_time_selection: re_uri::TimeSelection,
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

    ui.re_checkbox(create_web_viewer_url, "Web viewer URL")
        .on_hover_text("Create a link that can be opened directly in the browser.");

    ui.list_item_scope("url_selection_settings", |ui| {
        if let Some(fragments) = url.fragments_mut() {
            ui.list_item_collapsible_noninteractive_label("Customize selection", false, |ui| {
                let Fragment { focus, when } = fragments;

                let mut data_path = focus
                    .as_ref()
                    .map_or(String::new(), |path| path.to_string());
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Focused entity").value_text_mut(&mut data_path),
                );
                if data_path.is_empty() {
                    *focus = None;
                } else {
                    // TODO: handle parsing failure.
                    *focus = data_path.parse().ok();
                }

                let mut timeline = when.map_or(String::new(), |(timeline, _)| timeline.to_string());
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("timeline").value_text_mut(&mut timeline),
                );
                if timeline.is_empty() {
                    *when = None;
                } else {
                    // TODO: handle parsing failure.
                    // TODO: time range selection != time cursor
                    let timeline = TimelineName::from(timeline);
                    let time = when.map_or_else(
                        || {
                            TimeCell::new(
                                current_time_selection.timeline.typ(),
                                current_time_selection.range.min,
                            )
                        },
                        |(_, time)| time,
                    );
                    *when = Some((timeline, time));
                }

                // TODO: timeline selector

                let mut time = when.map_or(String::new(), |(_, time)| time.to_string());
                ui.list_item_flat_noninteractive(
                    // TODO: time selector
                    PropertyContent::new("Time").value_text_mut(&mut time),
                );
                if time.is_empty() {
                    fragments.when = None;
                } else if let Ok(time) = time.parse() {
                    // TODO: handle parsing failure.
                    let timeline = when
                        .map_or(*current_time_selection.timeline.name(), |(timeline, _)| {
                            timeline
                        });
                    *when = Some((timeline, time));
                }
            });
        }

        if let Some(url_time_range) = url.time_range_mut() {
            let mut entire_range = url_time_range.is_none();
            ui.list_item_flat_noninteractive(PropertyContent::new("Time range").value_fn(
                |ui, _| {
                    ui.selectable_value(&mut entire_range, true, "Entire recording");
                    ui.selectable_value(&mut entire_range, false, "Trim to selection");
                },
            ));

            if entire_range {
                *url_time_range = None;
            } else {
                *url_time_range = Some(current_time_selection);

                // TODO: controls.
                // TODO: snapshot control.
            }
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
