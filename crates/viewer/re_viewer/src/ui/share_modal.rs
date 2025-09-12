use egui::{AtomExt as _, IntoAtoms, NumExt as _};
use web_time::{Duration, Instant};

use re_log_types::AbsoluteTimeRange;
use re_redap_browser::EXAMPLES_ORIGIN;
use re_ui::{
    UiExt as _, icons,
    list_item::PropertyContent,
    modal::{ModalHandler, ModalWrapper},
};
use re_uri::Fragment;
use re_viewer_context::{
    DisplayMode, ItemCollection, RecordingConfig, StoreHub, UrlContext, ViewerContext,
};

use crate::open_url::ViewerOpenUrl;

const COPIED_FEEDBACK_DURATION: Duration = Duration::from_millis(500);

pub struct ShareModal {
    modal: ModalHandler,

    url: Option<ViewerOpenUrl>,
    create_web_viewer_url: bool,
    last_time_copied: Option<Instant>,
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
        }
    }
}

impl ShareModal {
    /// URL for the current screen, used as a starting point for the modal.
    fn current_url(
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
        rec_cfg: Option<&RecordingConfig>,
        selection: &ItemCollection,
    ) -> anyhow::Result<ViewerOpenUrl> {
        let url_context = {
            let time_ctrl = rec_cfg.map(|cfg| cfg.time_ctrl.read());
            UrlContext::from_context_expanded(display_mode, time_ctrl.as_deref(), selection)
        };
        ViewerOpenUrl::new(store_hub, url_context)
    }

    /// Opens the share modal with the current URL.
    pub fn open(
        &mut self,
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
        rec_cfg: Option<&RecordingConfig>,
        selection: &ItemCollection,
    ) -> anyhow::Result<()> {
        let url = Self::current_url(store_hub, display_mode, rec_cfg, selection)?;
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
        rec_cfg: Option<&RecordingConfig>,
        selection: &ItemCollection,
    ) {
        re_tracing::profile_function!();

        let url_for_current_screen = Self::current_url(store_hub, display_mode, rec_cfg, selection);
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
        ctx: &ViewerContext<'_>,
        ui: &egui::Ui,
        web_viewer_base_url: Option<&url::Url>,
    ) {
        let Some(url) = &mut self.url else {
            // Happens only if the modal is closed anyways.
            debug_assert!(!self.modal.is_open());
            return;
        };

        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Share"),
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

                    // TODO: make this editable.
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
                    url_settings_ui(ctx, ui, url, &mut self.create_web_viewer_url);
                });
            },
        );
    }
}

// TODO(andreas): why is this not a thing, should we make it one?
// TODO(andreas): What we _actually_ want is a group of toggles that are all vertically aligned.
fn selectable_value_with_min_width<'a, Value: PartialEq>(
    ui: &mut egui::Ui,
    min_width: f32,
    current_value: &mut Value,
    selected_value: Value,
    text: impl IntoAtoms<'a>,
) -> egui::Response {
    let checked = *current_value == selected_value;
    let mut response =
        ui.add(egui::Button::selectable(checked, text).min_size(egui::vec2(min_width, 0.0)));

    if response.clicked() && *current_value != selected_value {
        *current_value = selected_value;
        response.mark_changed();
    }
    response
}

const MIN_TOGGLE_WIDTH: f32 = 130.0;

fn url_settings_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    url: &mut ViewerOpenUrl,
    create_web_viewer_url: &mut bool,
) {
    ui.list_item_flat_noninteractive(PropertyContent::new("Link format").value_fn(|ui, _| {
        ui.selectable_toggle(|ui| {
            selectable_value_with_min_width(ui, MIN_TOGGLE_WIDTH, create_web_viewer_url, false, "Only source")
                .on_hover_text("Link works only in already opened viewers and not in the browser's address bar.");
            selectable_value_with_min_width(ui, MIN_TOGGLE_WIDTH, create_web_viewer_url, true, "Web viewer")
                .on_hover_text("Link works in the browser's address bar, opening a new viewer. You can still use this link in the native viewer as well.");
        });
    }));

    if let Some(url_time_range) = url.time_range_mut() {
        time_range_ui(ui, url_time_range, ctx.rec_cfg);
    }
    if let Some(fragments) = url.fragments_mut() {
        let timestamp_format = ctx.app_options().timestamp_format;
        time_cursor_ui(ui, fragments, timestamp_format, ctx.rec_cfg);
    }
}

fn time_range_ui(
    ui: &mut egui::Ui,
    url_time_range: &mut Option<re_uri::TimeSelection>,
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

    let mut entire_range = url_time_range.is_none();
    ui.list_item_flat_noninteractive(PropertyContent::new("Trim time range").value_fn(|ui, _| {
        ui.selectable_toggle(|ui| {
            selectable_value_with_min_width(
                ui,
                MIN_TOGGLE_WIDTH,
                &mut entire_range,
                true,
                "Entire recording",
            )
            .on_hover_text("Link will share the entire recording.");
            ui.add_enabled_ui(current_time_range_selection.is_some(), |ui| {
                selectable_value_with_min_width(
                    ui,
                    MIN_TOGGLE_WIDTH,
                    &mut entire_range,
                    false,
                    "Trim to selection",
                )
                .on_disabled_hover_text("No time range selected.")
                .on_hover_text("Link trims the recording to the selected time range.");
            });
        });
    }));

    if entire_range {
        *url_time_range = None;
    } else {
        *url_time_range = current_time_range_selection;
    }
}

fn time_cursor_ui(
    ui: &mut egui::Ui,
    fragments: &mut Fragment,
    timestamp_format: re_log_types::TimestampFormat,
    rec_cfg: &RecordingConfig,
) {
    let Fragment {
        selection: _, // We just always include the selection, not exposing it directly in the editor.
        when,
    } = fragments;

    let current_time_cursor = {
        let time_ctrl = rec_cfg.time_ctrl.read();
        time_ctrl
            .time_cell()
            .map(|cell| (*time_ctrl.timeline().name(), cell))
    };

    let mut any_time = when.is_some();
    ui.list_item_flat_noninteractive(PropertyContent::new("Time cursor").value_fn(|ui, _| {
        ui.selectable_toggle(|ui| {
            selectable_value_with_min_width(
                ui,
                MIN_TOGGLE_WIDTH,
                &mut any_time,
                false,
                "At the start",
            );
            ui.add_enabled_ui(current_time_cursor.is_some(), |ui| {
                let mut label = egui::Atoms::new("Current");
                if let Some((_, time_cell)) = current_time_cursor {
                    label.push_right(format_extra_toggle_info(
                        time_cell.format_compact(timestamp_format),
                    ));
                }

                selectable_value_with_min_width(ui, MIN_TOGGLE_WIDTH, &mut any_time, true, label)
                    .on_disabled_hover_text("No time selected.");
            });
        });
    }));
    if any_time {
        *when = current_time_cursor;
    } else {
        *when = None;
    }
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
    use re_viewer_context::{DisplayMode, Item, ItemCollection};

    use crate::{open_url::ViewerOpenUrl, ui::ShareModal};

    #[test]
    fn test_share_modal() {
        let test_ctx = TestContext::new();

        let timeline = re_log_types::Timeline::new_timestamp("pictime");

        let selection = Item::from(EntityPath::parse_forgiving("entity/path"));
        let origin = re_uri::Origin::from_str("rerun+http://example.com").unwrap();
        let dataset_id = re_tuid::Tuid::from_u128(0x182342300c5f8c327a7b4a6e5a379ac4);

        let modal = Arc::new(Mutex::new(ShareModal::default()));

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::Vec2::new(500.0, 300.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                test_ctx.run(ui.ctx(), |ctx| {
                    modal.lock().ui(ctx, ui, None);
                });
            });

        {
            let store_hub = test_ctx.store_hub.lock();
            modal
                .lock()
                .open(
                    &store_hub,
                    &DisplayMode::RedapServer(origin.clone()),
                    Some(&test_ctx.recording_config),
                    &ItemCollection::default(),
                )
                .unwrap();
        }
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
