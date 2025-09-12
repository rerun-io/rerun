use egui::{AtomExt as _, IntoAtoms as _, NumExt as _};
use web_time::{Duration, Instant};

use re_log_types::{AbsoluteTimeRange, TimeCell};
use re_redap_browser::EXAMPLES_ORIGIN;
use re_ui::{
    UiExt as _, icons,
    list_item::PropertyContent,
    modal::{ModalHandler, ModalWrapper},
};
use re_uri::Fragment;
use re_viewer_context::{DisplayMode, Item, RecordingConfig, StoreHub, UrlContext};

use crate::open_url::ViewerOpenUrl;

const COPIED_FEEDBACK_DURATION: Duration = Duration::from_millis(500);

pub struct ShareModal {
    modal: ModalHandler,

    url: Option<ViewerOpenUrl>,
    create_web_viewer_url: bool,
    last_time_copied: Option<Instant>,

    default_expanded: bool,
}

impl Default for ShareModal {
    fn default() -> Self {
        // Put this on an extra line, otherwise if this isn't wasm32, clippy
        // thinks that this default impl is derivable.
        let create_web_viewer_url = cfg!(target_arch = "wasm32");

        Self {
            modal: ModalHandler::default(),

            url: None,
            create_web_viewer_url,
            last_time_copied: None,
            default_expanded: false,
        }
    }
}

impl ShareModal {
    /// URL for the current screen, used as a starting point for the modal.
    fn current_url(
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
    ) -> anyhow::Result<ViewerOpenUrl> {
        // TODO: add more to the url context.
        ViewerOpenUrl::new(store_hub, UrlContext::new(display_mode.clone()))
    }

    /// Opens the share modal with the current URL.
    pub fn open(&mut self, store_hub: &StoreHub, display_mode: &DisplayMode) -> anyhow::Result<()> {
        let url = Self::current_url(store_hub, display_mode)?;
        self.open_with_url(url);
        Ok(())
    }

    /// Opens the share modal with the given URL.
    fn open_with_url(&mut self, url: ViewerOpenUrl) {
        self.url = Some(url);
        self.modal.open();
    }

    /// Button that opens the share popup.
    pub fn button_ui(
        &mut self,
        ui: &mut egui::Ui,
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
    ) {
        re_tracing::profile_function!();

        let url_for_current_screen = Self::current_url(store_hub, display_mode);
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
                    self.open_with_url(url);
                }
            }
        }
    }

    /// Draws the share modal dialog if its open.
    pub fn ui(
        &mut self,
        ui: &egui::Ui,
        web_viewer_base_url: Option<&url::Url>,
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
                        web_viewer_base_url
                    } else {
                        None
                    };
                    let mut url_string = url.sharable_url(web_viewer_base_url).unwrap_or_default();

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
                        self.default_expanded,
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
    default_expanded: bool,
    rec_cfg: &RecordingConfig,
) {
    ui.list_item_flat_noninteractive(
        PropertyContent::new("Web viewer URL").value_bool_mut(create_web_viewer_url),
    );

    if let Some(url_time_range) = url.time_range_mut() {
        time_range_ui(ui, url_time_range, timestamp_format, rec_cfg);
    }

    if let Some(fragments) = url.fragments_mut() {
        fragments_ui(
            ui,
            fragments,
            timestamp_format,
            current_selection,
            default_expanded,
            rec_cfg,
        );
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
    ui.list_item_flat_noninteractive(PropertyContent::new("Trim time range").value_fn(|ui, _| {
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
    default_expanded: bool,
    rec_cfg: &RecordingConfig,
) {
    ui.list_item_collapsible_noninteractive_label("Selection", default_expanded, |ui| {
        let Fragment { selection, when } = fragments;

        let mut any_selection = selection.is_some();
        let current_selection = current_selection.and_then(|selection| selection.to_data_path());
        ui.list_item_flat_noninteractive(PropertyContent::new("Selection").value_fn(|ui, _| {
            ui.selectable_toggle(|ui| {
                ui.selectable_value(&mut any_selection, false, "None");
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
                    ui.selectable_value(&mut any_selection, true, label)
                        .on_disabled_hover_text(disabled_reason)
                });
            });
        }));
        if any_selection {
            *selection = current_selection;
        } else {
            *selection = None;
        }

        let current_time_cursor = {
            let time_ctrl = rec_cfg.time_ctrl.read();
            time_ctrl
                .time_cell()
                .map(|cell| (*time_ctrl.timeline().name(), cell))
        };
        let mut any_time = when.is_some();
        ui.list_item_flat_noninteractive(PropertyContent::new("Time").value_fn(|ui, _| {
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
        }));
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

#[cfg(test)]
mod tests {
    use std::{str::FromStr as _, sync::Arc};

    use parking_lot::Mutex;
    use re_chunk::EntityPath;
    use re_log_types::{AbsoluteTimeRangeF, TimeCell, external::re_tuid};
    use re_test_context::TestContext;
    use re_viewer_context::{DisplayMode, Item};

    use crate::{open_url::ViewerOpenUrl, ui::ShareModal};

    #[test]
    fn test_share_modal() {
        let test_ctx = TestContext::new();

        let timeline = re_log_types::Timeline::new_timestamp("pictime");

        let selection = Item::from(EntityPath::parse_forgiving("entity/path"));
        let origin = re_uri::Origin::from_str("rerun+http://example.com").unwrap();
        let dataset_id = re_tuid::Tuid::from_u128(0x182342300c5f8c327a7b4a6e5a379ac4);

        let modal = Arc::new(Mutex::new(ShareModal::default()));
        modal.lock().default_expanded = true;

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::Vec2::new(500.0, 300.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                modal.lock().ui(
                    ui,
                    None,
                    re_log_types::TimestampFormat::Utc,
                    Some(&selection),
                    &test_ctx.recording_config,
                );
            });

        let store_hub = test_ctx.store_hub.lock();
        modal
            .lock()
            .open(&store_hub, &DisplayMode::RedapServer(origin.clone()))
            .unwrap();
        harness.run();
        harness.snapshot("share_modal__server_url");

        modal.lock().url = Some(ViewerOpenUrl::RedapDatasetPartition(
            re_uri::DatasetPartitionUri {
                origin: origin.clone(),
                dataset_id,
                partition_id: "partition_id".to_owned(),
                time_range: None,
                fragment: re_uri::Fragment::default(),
            },
        ));
        harness.run_steps(2); // Force running two steps to ensure relayouting happens. TODO(andreas): Why is this needed?
        harness.snapshot("share_modal__dataset_partition_url");

        // Set the timeline so it shows up on the dialog.
        {
            test_ctx.set_active_timeline(timeline);
            let mut time_ctrl = test_ctx.recording_config.time_ctrl.write();
            time_ctrl.set_loop_selection(AbsoluteTimeRangeF::new(0.0, 1000.0));
        }

        modal.lock().url = Some(ViewerOpenUrl::RedapDatasetPartition(
            re_uri::DatasetPartitionUri {
                origin: origin.clone(),
                dataset_id,
                partition_id: "partition_id".to_owned(),
                time_range: Some(re_uri::TimeSelection {
                    timeline,
                    range: re_log_types::AbsoluteTimeRange::new(0, 1000),
                }),
                fragment: re_uri::Fragment {
                    selection: selection.to_data_path(),
                    when: Some((*timeline.name(), TimeCell::new(timeline.typ(), 234))),
                },
            },
        ));
        harness.run();
        harness.snapshot("share_modal__dataset_partition_url_with_time_range");
    }
}
