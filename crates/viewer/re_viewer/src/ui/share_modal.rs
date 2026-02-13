use egui::{AtomExt as _, IntoAtoms, NumExt as _};
use re_redap_browser::EXAMPLES_ORIGIN;
use re_ui::list_item::PropertyContent;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{UiExt as _, icons};
use re_uri::Fragment;
use re_viewer_context::open_url::ViewerOpenUrl;
use re_viewer_context::{DisplayMode, ItemCollection, StoreHub, TimeControl, ViewerContext};

pub struct ShareModal {
    modal: ModalHandler,

    url: Option<ViewerOpenUrl>,
    create_web_viewer_url: bool,

    /// Whether to show feedback that the link just has been copied to the clipboard.
    ///
    /// This is shown on pressing the copy link button and reset when the button is no longer hovered.
    show_copied_feedback: bool,
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
            show_copied_feedback: false,
        }
    }
}

impl ShareModal {
    /// URL for the current screen, used as a starting point for the modal.
    fn current_url(
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
        time_ctrl: Option<&TimeControl>,
        selection: &ItemCollection,
    ) -> anyhow::Result<ViewerOpenUrl> {
        ViewerOpenUrl::from_context_expanded(store_hub, display_mode, time_ctrl, selection)
    }

    /// Opens the share modal with the current URL.
    pub fn open(
        &mut self,
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
        time_ctrl: Option<&TimeControl>,
        selection: &ItemCollection,
    ) -> anyhow::Result<()> {
        let url = Self::current_url(store_hub, display_mode, time_ctrl, selection)?;
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
        time_ctrl: Option<&TimeControl>,
        selection: &ItemCollection,
    ) {
        re_tracing::profile_function!();

        let url_for_current_screen =
            Self::current_url(store_hub, display_mode, time_ctrl, selection);
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
            re_log::debug_assert!(!self.modal.is_open());
            return;
        };

        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Share"),
            |ui| {
                let panel_max_height = (ui.ctx().content_rect().height() - 100.0)
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
                    let url_string = url.sharable_url(web_viewer_base_url).unwrap_or_default();

                    // We don't actually want to edit the URL, but text edit is stylewise what we want here.
                    // TODO(andreas): This is slightly glitchy: you can still type and we forget about it immediately.
                    // Ideally we'd keep this interactive but don't allow text edits, but `TextEdit` doesn't have this option yet.
                    let mut url_for_text_edit = url_string.clone();
                    egui::TextEdit::singleline(&mut url_for_text_edit)
                        .hint_text("<can't share link>") // No known way to get into this situation.
                        .text_color(ui.style().visuals.strong_text_color())
                        .desired_width(f32::INFINITY) // Take up the entire space.
                        .show(ui);

                    url_string
                };

                let copy_link_label = if self.show_copied_feedback {
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
                    self.show_copied_feedback = true;
                } else if !copy_link_response.hovered() {
                    self.show_copied_feedback = false;
                }

                ui.list_item_scope("share_dialog_url_settings", |ui| {
                    url_settings_ui(ctx, ui, url, &mut self.create_web_viewer_url);
                });
            },
        );
    }
}

// TODO(andreas): why is this not a thing, should we make it one?
// TODO(andreas): What we _actually_ want is a group of toggles that are all vertically aligned and take up the entire width.
fn selectable_value_with_min_width<'a, Value: PartialEq>(
    ui: &mut egui::Ui,
    min_width: f32,
    current_value: &mut Value,
    selected_value: Value,
    text: impl IntoAtoms<'a>,
) -> egui::Response {
    let checked = *current_value == selected_value;
    let mut response = ui.add(
        egui::Button::selectable(checked, text)
            .wrap_mode(egui::TextWrapMode::Truncate)
            .min_size(egui::vec2(min_width, 0.0)),
    );

    if response.clicked() && *current_value != selected_value {
        *current_value = selected_value;
        response.mark_changed();
    }
    response
}

fn selectable_value_with_available_width<'a, Value: PartialEq>(
    ui: &mut egui::Ui,
    current_value: &mut Value,
    selected_value: Value,
    text: impl IntoAtoms<'a>,
) -> egui::Response {
    selectable_value_with_min_width(
        ui,
        ui.available_width(),
        current_value,
        selected_value,
        text,
    )
}

const MIN_TOGGLE_WIDTH_RH: f32 = 120.0;

fn url_settings_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    url: &mut ViewerOpenUrl,
    create_web_viewer_url: &mut bool,
) {
    ui.list_item_flat_noninteractive(PropertyContent::new("Link format").value_fn(|ui, _| {
        ui.selectable_toggle(|ui| {
            selectable_value_with_min_width(ui, MIN_TOGGLE_WIDTH_RH, create_web_viewer_url, false, "Only source")
                .on_hover_text("Link works only in already opened viewers and not in the browser's address bar.");
            selectable_value_with_available_width(ui, create_web_viewer_url, true, "Web viewer")
                .on_hover_text("Link works in the browser's address bar, opening a new viewer. You can still use this link in the native viewer as well.");
        });
    }));

    if let Some(fragments) = url.fragment_mut() {
        ui.add_space(8.0);

        let timestamp_format = ctx.app_options().timestamp_format;
        fragment_ui(ui, fragments, timestamp_format, ctx.time_ctrl);
    }
}

fn fragment_ui(
    ui: &mut egui::Ui,
    fragments: &mut Fragment,
    timestamp_format: re_log_types::TimestampFormat,
    time_ctrl: &TimeControl,
) {
    let Fragment {
        selection: _, // We just always include the selection, not exposing it directly in the editor.
        when,
        time_selection,
    } = fragments;

    let current_time_cursor = {
        time_ctrl
            .time_cell()
            .map(|cell| (*time_ctrl.timeline_name(), cell))
    };

    let current_time_selection = time_ctrl.time_selection().and_then(|time_selection| {
        Some(re_uri::TimeSelection {
            timeline: *time_ctrl.timeline()?,
            range: time_selection.to_int(),
        })
    });

    let mut any_time = when.is_some();
    ui.list_item_flat_noninteractive(PropertyContent::new("Time cursor").value_fn(|ui, _| {
        ui.selectable_toggle(|ui| {
            selectable_value_with_min_width(
                ui,
                MIN_TOGGLE_WIDTH_RH,
                &mut any_time,
                false,
                "At the start",
            );
            ui.add_enabled_ui(current_time_cursor.is_some(), |ui| {
                let mut label = egui::Atoms::new(egui::Atom::from("Current"));
                if let Some((_, time_cell)) = current_time_cursor {
                    label.push_right({
                        let time = time_cell.format(timestamp_format);
                        egui::RichText::new(time).weak().small().atom_shrink(true)
                    });
                }
                label.push_left(egui::Atom::grow());
                label.push_right(egui::Atom::grow());

                selectable_value_with_available_width(ui, &mut any_time, true, label)
                    .on_disabled_hover_text("No time selected.");
            });
        });
    }));

    ui.add_space(8.0);

    let mut any_selection = time_selection.is_some();
    ui.list_item_flat_noninteractive(PropertyContent::new("Time selection").value_fn(|ui, _| {
        ui.selectable_toggle(|ui| {
            selectable_value_with_min_width(
                ui,
                MIN_TOGGLE_WIDTH_RH,
                &mut any_selection,
                false,
                "No selection",
            );
            ui.add_enabled_ui(current_time_selection.is_some(), |ui| {
                let mut label = egui::Atoms::new(egui::Atom::from("Current"));
                if let Some(time_selection) = &current_time_selection {
                    label.push_right({
                        egui::RichText::new(time_selection.format(timestamp_format))
                            .weak()
                            .small()
                            .atom_shrink(true)
                    });
                }
                label.push_left(egui::Atom::grow());
                label.push_right(egui::Atom::grow());

                selectable_value_with_available_width(ui, &mut any_selection, true, label)
                    .on_disabled_hover_text("No time selected.");
            });
        });
    }));
    if any_time {
        *when = current_time_cursor;
    } else {
        *when = None;
    }

    if any_selection {
        *time_selection = current_time_selection;
    } else {
        *time_selection = None;
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;
    use std::sync::Arc;

    use parking_lot::Mutex;
    use re_chunk::EntityPath;
    use re_log_types::external::re_tuid;
    use re_log_types::{AbsoluteTimeRangeF, TimeCell};
    use re_test_context::TestContext;
    use re_viewer_context::open_url::ViewerOpenUrl;
    use re_viewer_context::{DisplayMode, Item, ItemCollection, TimeControlCommand};

    use crate::ui::ShareModal;

    #[test]
    fn test_share_modal() {
        let mut test_ctx = TestContext::new();

        let timeline = re_log_types::Timeline::new_timestamp("pictime");

        // Log some entity so our timeline exists.
        test_ctx.log_entity(EntityPath::from("points"), |builder| {
            builder.with_archetype(
                re_chunk::RowId::new(),
                [(timeline, re_chunk::TimeInt::ZERO)],
                &re_sdk_types::archetypes::Points2D::new([(0., 0.), (1., 1.)]),
            )
        });

        let selection = Item::from(EntityPath::parse_forgiving("entity/path"));
        let origin = re_uri::Origin::from_str("rerun+http://example.com").unwrap();
        let dataset_id = re_tuid::Tuid::from_u128(0x182342300c5f8c327a7b4a6e5a379ac4);

        let modal = Arc::new(Mutex::new(ShareModal::default()));

        {
            let store_hub = test_ctx.store_hub.lock();
            modal
                .lock()
                .open(
                    &store_hub,
                    &DisplayMode::RedapServer(origin.clone()),
                    Some(&test_ctx.time_ctrl.read()),
                    &ItemCollection::default(),
                )
                .unwrap();
        }

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::Vec2::new(500.0, 300.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                test_ctx.run(ui.ctx(), |ctx| {
                    modal.lock().ui(ctx, ui, None);
                });

                test_ctx.handle_system_commands(ui.ctx());
            });
        harness.snapshot("share_modal__server_url");

        modal.lock().url = Some(ViewerOpenUrl::RedapDatasetSegment(
            re_uri::DatasetSegmentUri {
                origin: origin.clone(),
                dataset_id,
                segment_id: "segment_id".to_owned(),
                fragment: re_uri::Fragment::default(),
            },
        ));
        harness.run_steps(2); // Force running two steps to ensure relayouting happens. TODO(andreas): Why is this needed?
        harness.snapshot("share_modal__dataset_segment_url");

        // Set the timeline so it shows up on the dialog.
        test_ctx.send_time_commands(
            test_ctx.active_store_id(),
            [
                TimeControlCommand::SetActiveTimeline(*timeline.name()),
                TimeControlCommand::SetTime(re_chunk::TimeInt::ZERO.into()),
                TimeControlCommand::SetTimeSelection(AbsoluteTimeRangeF::new(0.0, 1000.0).to_int()),
            ],
        );

        harness.run();

        modal.lock().url = Some(ViewerOpenUrl::RedapDatasetSegment(
            re_uri::DatasetSegmentUri {
                origin: origin.clone(),
                dataset_id,
                segment_id: "segment_id".to_owned(),
                fragment: re_uri::Fragment {
                    selection: selection.to_data_path(),
                    when: Some((*timeline.name(), TimeCell::new(timeline.typ(), 234))),
                    time_selection: Some(re_uri::TimeSelection {
                        timeline,
                        range: re_log_types::AbsoluteTimeRange::new(0, 1000),
                    }),
                },
            },
        ));
        harness.run();
        harness.snapshot("share_modal__dataset_segment_url_with_time_range");
    }
}
