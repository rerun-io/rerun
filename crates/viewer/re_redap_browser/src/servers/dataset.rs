use datafusion::sql::TableReference;
use egui::AtomExt as _;
use re_dataframe_ui::TableCellKind;
use re_format::format_uint;
use re_log_types::external::re_types_core::SegmentId;
use re_log_types::{EntityPathPart, EntryId, Timestamp};
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_quota_channel::send_crossbeam;
use re_redap_client::{ApiError, Asset, ConnectionHandle, DEFAULT_ASSET_TASK_TIMEOUT};
use re_ui::egui_ext::card_layout::{CardLayout, CardLayoutItem};
use re_ui::time::{short_duration_text, short_duration_ui};
use re_ui::{
    DesignTokens, ReButton, ServerValue, TabBar, TableCommand, TableCommandKind,
    TableCommandSender as _, UiExt as _, icons,
};
use re_uri::DatasetResource;
use re_viewer_context::{
    AppContext, SystemCommand, SystemCommandSender as _, TableReference as ViewerTableReference,
    ViewStates,
};

use crate::asset_registration::{AssetRegistration, RegistrationState};
use crate::context::Context;
use crate::entry_meta::{AssetsRef, EntryMeta, EntryMetaQuery};
use crate::register_asset_modal::{AssetSlots, AssetTarget};
use crate::servers::Command;
use crate::{
    Server,
    entries::{Dataset, Entry, EntryInner},
};

const PADDING: f32 = 16.0;

/// Font size of a dataset's name.
const HEADER_FONT_SIZE: f32 = 19.0;

/// The height the dataset's name is fitted into.
///
/// The name is centered in it, so the header height does not depend on the font metrics.
const HEADER_HEIGHT: f32 = 24.0;

/// Space between the breadcrumb path and the dataset's name.
const BREADCRUMB_TO_NAME_SPACE: f32 = 4.0;

/// Space between the dataset's name and the tabs under it.
///
/// `item_spacing.y` is zeroed here, so this is the whole gap apart from the padding a tab keeps
/// above its label.
const NAME_TO_TABS_SPACE: f32 = 4.0;

/// Space between the two lines of an asset card: its name and the metadata line under it.
const CARD_LINE_SPACE: f32 = 8.0;

/// Space between the dataset's name and the refresh button after it.
const NAME_TO_REFRESH_SPACE: f32 = 8.0;

/// Space between a value and the word naming it in the segment table's toolbar.
///
/// The two are separate labels, so they can be colored differently, and the space between them
/// cannot be part of the text.
const META_WORD_SPACE: f32 = 4.0;

impl Server {
    /// Where to fetch metadata about one of this server's datasets from.
    fn entry_meta_query<'a>(
        &'a self,
        egui_ctx: &'a egui::Context,
        dataset: &'a Dataset,
    ) -> EntryMetaQuery<'a> {
        EntryMetaQuery {
            runtime: &self.runtime,
            egui_ctx,
            connection: &self.connection,
            dataset_id: dataset.id(),
        }
    }

    pub fn dataset_entry_ui(
        &self,
        app_ctx: &AppContext<'_>,
        ctx: &Context<'_>,
        ui: &mut egui::Ui,
        dataset: &Dataset,
        table_blueprints: &re_dataframe_ui::TableBlueprints,
        view_states: &mut ViewStates,
        kind: Option<re_viewer_context::EntryKind>,
    ) {
        let resource = match kind {
            Some(re_viewer_context::EntryKind::Dataset(resource)) => resource,
            Some(re_viewer_context::EntryKind::Table) | None => DatasetResource::default(),
        };

        ui.push_id((&dataset.origin, dataset.id()), |ui| {
            // The header, the tabs and the content under them each add their own space around
            // themselves, so `item_spacing.y` must not add to it.
            ui.spacing_mut().item_spacing.y = 0.0;

            let name = dataset.name();

            let path = name
                .as_str()
                .split(re_uri::DATASET_HIERARCHY_SEPARATOR)
                .collect::<Vec<_>>();

            ui.horizontal(|ui| {
                ui.add_space(PADDING);
                ui.vertical(|ui| {
                    // Every gap in the header is set explicitly below, not left to `item_spacing`.
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(PADDING);

                    if path.len() > 1 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (idx, part) in path.iter().enumerate() {
                                let last = idx == path.len() - 1;
                                if last {
                                    ui.strong(*part);
                                } else {
                                    ui.label(format!("{part} / "));
                                }
                            }
                        });
                        ui.add_space(BREADCRUMB_TO_NAME_SPACE);
                    }

                    if let Some(name) = path.last() {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), HEADER_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = NAME_TO_REFRESH_SPACE;
                                ui.label(
                                    egui::RichText::new(*name).size(HEADER_FONT_SIZE).strong(),
                                );
                                refresh_button_ui(
                                    ui,
                                    app_ctx,
                                    dataset,
                                    self.segments_queried_at(ui, dataset),
                                );
                            },
                        );
                    }

                    ui.add_space(NAME_TO_TABS_SPACE);
                });
            });

            let mut new_resource = resource;
            TabBar::new(ui)
                .selectable_value(&mut new_resource, DatasetResource::Segments, "Segments")
                .selectable_value(&mut new_resource, DatasetResource::Assets, "Assets");

            if new_resource != resource {
                app_ctx
                    .command_sender()
                    .send_system(SystemCommand::SetRoute(
                        re_viewer_context::Route::RedapEntry {
                            origin: dataset.origin.clone(),
                            entry_id: dataset.id(),
                            kind: Some(re_viewer_context::EntryKind::Dataset(new_resource)),
                        },
                    ));
            }

            match new_resource {
                DatasetResource::Segments => {
                    self.segments_ui(app_ctx, ui, dataset, table_blueprints, view_states);
                }
                DatasetResource::Assets => {
                    self.assets_ui(ui, app_ctx, ctx, dataset);
                }
            }
        });
    }

    /// When the client last fetched the segment table shown in the segments tab, or `None` if it
    /// has not been fetched yet.
    fn segments_queried_at(&self, ui: &egui::Ui, dataset: &Dataset) -> Option<Timestamp> {
        re_dataframe_ui::DataFusionTableWidget::queried_at(
            ui,
            &self.tables_session_ctx,
            TableReference::bare(dataset.name().to_string()),
        )
    }

    /// How many segments the dataset holds, shown at the left end of the segment table's toolbar.
    ///
    /// This counts the whole dataset, not the rows of the table, which a filter can narrow down.
    fn segment_count_ui(&self, ui: &mut egui::Ui, dataset: &Dataset) {
        let EntryMeta { segments } = dataset
            .requests()
            .meta(self.entry_meta_query(ui.ctx(), dataset));

        let segments = segments.get().copied();

        tab_meta_line_ui(
            ui,
            &[MetaTerm {
                value: segments.map(format_uint),
                label: if segments == Some(1) {
                    "segment"
                } else {
                    "segments"
                },
            }],
        );
    }

    fn segments_ui(
        &self,
        app_ctx: &AppContext<'_>,
        ui: &mut egui::Ui,
        dataset: &Dataset,
        table_blueprints: &re_dataframe_ui::TableBlueprints,
        view_states: &mut ViewStates,
    ) {
        const RECORDING_LINK_COLUMN_NAME: &str = "recording link";

        re_dataframe_ui::DataFusionTableWidget::new(
            self.tables_session_ctx.clone(),
            TableReference::bare(dataset.name().to_string()),
            ViewerTableReference::RedapEntry {
                origin: dataset.origin.clone(),
                entry_id: dataset.id(),
            },
        )
        .toolbar_summary(|ui| self.segment_count_ui(ui, dataset))
        .additional_column_heuristics(|desc, mut column| {
            // TODO(andreas): we should not operate on display name as much since this can be very brittle.
            // TODO(andreas): Most of these heuristics could just be always applied so all tables profit from then.

            let mut name = column.display_name();

            // Strip the prefix and remove underscores only for base columns, not properties.
            name = name
                .strip_prefix("rerun_")
                .map(|name| name.replace('_', " "))
                .unwrap_or(name);

            let default_visible = if desc.entity_path().is_some_and(|entity_path| {
                entity_path.starts_with(&std::iter::once(EntityPathPart::properties()).collect())
            }) {
                true
            } else {
                desc.display_name().as_str() == RECORDING_LINK_COLUMN_NAME
            };

            column = column
                .with_default_display_name(name)
                .with_default_visibility(default_visible);

            if desc.display_name().as_str() == RECORDING_LINK_COLUMN_NAME {
                column = column.with_default_cell_kind(TableCellKind::Link);
            }

            column
        })
        .generate_segment_links(
            RECORDING_LINK_COLUMN_NAME.into(),
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME.into(),
            self.origin().clone(),
            dataset.id(),
        )
        .show(app_ctx, &self.runtime, ui, table_blueprints, view_states);
    }

    /// Lists the assets of a dataset, or says why there is nothing to list.
    fn assets_ui(
        &self,
        ui: &mut egui::Ui,
        app_ctx: &AppContext<'_>,
        ctx: &Context<'_>,
        dataset: &Dataset,
    ) {
        let assets = dataset.requests().assets(
            self.entry_meta_query(ui.ctx(), dataset),
            dataset.asset_dataset(),
        );

        // We want to keep the registration around while we're still refetching the asset list
        // so it doesn't flash in and out between pending and in the list.
        let waiting_for_assets = matches!(assets, ServerValue::Pending { .. });

        let pending_registrations: Vec<_> = self
            .asset_registrations
            .for_dataset(dataset.id())
            .filter(|registration| {
                waiting_for_assets || !matches!(registration.state, RegistrationState::Registered)
            })
            .filter(|registration| {
                // An asset that stays registered after the server refused it is in the list, and
                // its card shows the failure and the reason instead.
                registration.failed_asset().is_none_or(|asset_id| {
                    assets
                        .get()
                        .is_none_or(|assets| assets.iter().all(|asset| &asset.id != asset_id))
                })
            })
            .collect();

        let asset_target = AssetTarget {
            origin: dataset.origin.clone(),
            dataset_id: dataset.id(),
        };
        let asset_slots = self.asset_slots(dataset.id(), &assets);

        // A registration belongs in the list even before the dataset has an asset to show.
        let no_assets_yet = assets
            .get()
            .is_some_and(|asset_list| asset_list.is_empty() && pending_registrations.is_empty());

        // The toolbar keeps its own space above it, so only the card that stands in for the empty
        // list needs room at the top.
        let top_margin = if no_assets_yet { PADDING as i8 } else { 0 };

        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: PADDING as i8,
                right: PADDING as i8,
                top: top_margin,

                // Keeps the limits line off the bottom edge.
                bottom: 4,
            })
            .show(ui, |ui| {
                if no_assets_yet {
                    no_assets_ui(ui, ctx, &asset_target, &asset_slots);
                } else if assets.get().is_some() {
                    self.asset_card_list(
                        ui,
                        app_ctx,
                        ctx,
                        dataset,
                        &assets,
                        &pending_registrations,
                        &asset_target,
                        &asset_slots,
                    );
                } else if let Some(err) = assets.get_err() {
                    ui.error_label(err.to_string());
                } else {
                    ui.loading_indicator("waiting for assets");
                }
            });
    }

    /// How full a dataset's asset slots are, given its asset list.
    fn asset_slots(
        &self,
        dataset_id: EntryId,
        assets: &ServerValue<AssetsRef, ApiError>,
    ) -> AssetSlots {
        AssetSlots {
            registered_count: assets.get().map_or(0, |assets| assets.len()),
            pending_source_uris: self.asset_registrations.pending_source_uris(dataset_id),
        }
    }

    /// How full a dataset's asset slots are, asking for its asset list if nothing has yet.
    pub(super) fn dataset_asset_slots(
        &self,
        egui_ctx: &egui::Context,
        dataset_id: EntryId,
    ) -> AssetSlots {
        let assets = match self.find_entry(dataset_id).map(Entry::inner) {
            Some(Ok(EntryInner::Dataset(dataset))) => dataset.requests().assets(
                self.entry_meta_query(egui_ctx, dataset),
                dataset.asset_dataset(),
            ),

            // We don't know the dataset, so only its pending registrations count.
            _ => ServerValue::Completed(AssetsRef::default()),
        };

        self.asset_slots(dataset_id, &assets)
    }

    /// One card per asset and per pending or failed registration, with the register button and the
    /// limits that apply to them.
    fn asset_card_list(
        &self,
        ui: &mut egui::Ui,
        app_ctx: &AppContext<'_>,
        ctx: &Context<'_>,
        dataset: &Dataset,
        assets: &ServerValue<AssetsRef, ApiError>,
        registrations: &[&AssetRegistration],
        asset_target: &AssetTarget,
        asset_slots: &AssetSlots,
    ) {
        let tokens = ui.tokens();

        // The same toolbar margins the segments tab gets from `DataFusionTableWidget`, so the
        // content does not move when switching tabs. The horizontal inset is already on the frame
        // around this.
        let toolbar_frame = egui::Frame::new().inner_margin(egui::Margin::symmetric(
            0,
            re_ui::TAB_TOOLBAR_MARGIN_Y as i8,
        ));

        toolbar_frame.show(ui, |ui| {
            egui::Sides::new().show(
                ui,
                |ui| {
                    ui.set_height(re_ui::TAB_TOOLBAR_HEIGHT);

                    // Counted like the register button and the modal count it, including pending
                    // registrations.
                    let asset_count = assets.get().map(|_| asset_slots.taken().to_string());

                    let total_bytes = assets
                        .get()
                        .map(|a| a.iter().map(|a| a.size as f64).sum::<f64>())
                        .map(re_format::format_bytes);

                    // There is no total size until the asset list has arrived, so while it is
                    // pending only the count is shown, with its loading indicator.
                    let mut terms = vec![MetaTerm {
                        value: asset_count,
                        label: "assets",
                    }];
                    if let Some(total_bytes) = total_bytes {
                        terms.push(MetaTerm {
                            value: Some(total_bytes),
                            label: "total",
                        });
                    }
                    tab_meta_line_ui(ui, &terms);

                    if let Some(err) = assets.get_err() {
                        ui.error_label(err.to_string());
                    }
                },
                |ui| {
                    ui.set_height(re_ui::TAB_TOOLBAR_HEIGHT);
                    register_asset_button(ctx, asset_target, asset_slots, ui);
                },
            );
        });

        // The frame's own bottom margin is the whole gap down to the cards.
        ui.spacing_mut().item_spacing.y = 0.0;

        // The limits line sits in a bottom panel, so the card list gets the height that is left
        // and its scroll area scrolls within that height.
        egui::Panel::bottom(ui.id().with("asset_limits"))
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add(egui::Label::new(
                    egui::RichText::new(asset_slots.limit_rules().join(" · "))
                        .color(ui.visuals().weak_text_color()),
                ));
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| match assets.get() {
                // The header already says whether the list is still pending or failed.
                None => {}

                Some(assets) => {
                    let inner_margin =
                        egui::Margin::same(tokens.table_grid_view_card_inner_margin as i8);
                    let card_frame = egui::Frame::new()
                        .inner_margin(inner_margin)
                        .fill(tokens.card_fill)
                        .stroke(tokens.card_stroke)
                        .corner_radius(tokens.table_grid_view_card_corner_radius);

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let old_item_spacing = std::mem::replace(
                            &mut ui.spacing_mut().item_spacing,
                            egui::vec2(0.0, 11.0),
                        );

                        // Registrations come first, at the top of the list.
                        let card_width = ui.available_width();
                        let items = std::iter::chain(
                            registrations.iter().map(|registration| CardLayoutItem {
                                min_width: card_width,
                                frame: Some(card_frame.stroke(egui::Stroke::new(
                                    1.0,
                                    registration_outline_color(&registration.state, tokens),
                                ))),

                                // A registration has nothing to open yet.
                                clickable: Some(false),
                            }),
                            assets.iter().map(|asset| CardLayoutItem {
                                min_width: card_width,
                                frame: (!asset.is_registered()).then(|| {
                                    card_frame.stroke(egui::Stroke::new(
                                        1.0,
                                        asset_outline_color(asset, tokens),
                                    ))
                                }),

                                // Only a registered asset can be opened.
                                clickable: Some(asset.is_registered()),
                            }),
                        )
                        .collect();

                        let clicked = CardLayout::new(items, card_frame)
                            .hover_fill(tokens.card_hover_fill)
                            .hover_stroke(tokens.card_hover_stroke)
                            .show(ui, |ui, idx, _hovered| {
                                // The page zeroes `item_spacing` to keep its own gaps explicit,
                                // but a card lays out its own lines and needs vertical spacing.
                                ui.spacing_mut().item_spacing = old_item_spacing;
                                ui.spacing_mut().item_spacing.y = CARD_LINE_SPACE;

                                if let Some(registration) = registrations.get(idx) {
                                    asset_registration_ui(ui, ctx, dataset, registration);
                                } else if let Some(asset) = assets.get(idx - registrations.len()) {
                                    self.asset_ui(ui, app_ctx, ctx, dataset, asset);
                                }
                            });

                        // A card's outline is drawn on its edge, so the scroll area keeps the
                        // outline's width under the last card instead of clipping it in half.
                        // `item_spacing.y` must not add to that.
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add_space(card_frame.stroke.width);

                        // Clicking a card opens the asset. Only registered assets are clickable,
                        // so the index is always past the registrations.
                        if let Some(asset) = clicked
                            .and_then(|idx| idx.checked_sub(registrations.len()))
                            .and_then(|idx| assets.get(idx))
                        {
                            open_asset_ui(ui, dataset, asset, false);
                        }
                    });
                }
            });
    }

    fn asset_ui(
        &self,
        ui: &mut egui::Ui,
        app_ctx: &AppContext<'_>,
        ctx: &Context<'_>,
        dataset: &Dataset,
        asset: &Asset,
    ) {
        egui::Sides::new()
            .shrink_left()
            .truncate()
            .height(card_height(ui))
            .show(
                ui,
                |ui| {
                    ui.vertical(|ui| {
                        let tokens = ui.tokens();

                        if self.asset_unregistrations.contains(dataset.id(), &asset.id) {
                            title_with_pill_ui(
                                ui,
                                asset.id.as_str(),
                                "unregistering",
                                pending_fg_color(tokens),
                            );

                            ui.horizontal(|ui| {
                                ui.inline_loading_indicator("unregistering asset");
                                ui.weak("Waiting for the server to unregister it");
                            });
                        } else if asset.is_registered() {
                            ui.label(egui::RichText::new(asset.id.as_str()).heading().strong());

                            asset_meta_ui(ui, app_ctx, asset);
                        } else {
                            let color = asset_fg_color(asset, tokens);
                            let pill_text = if asset.has_failed() {
                                "error"
                            } else {
                                "pending"
                            };

                            title_with_pill_ui(ui, asset.id.as_str(), pill_text, color);

                            if asset.has_failed() {
                                ui.horizontal(|ui| {
                                    // The manifest stores the status of a registration but not the
                                    // reason it failed, so after a reload all the viewer knows is
                                    // that it failed.
                                    let reason = self
                                        .asset_registrations
                                        .failure_reason(dataset.id(), &asset.id)
                                        .map_or_else(
                                            || "The server failed to register it".to_owned(),
                                            failure_reason,
                                        );
                                    ui.label(egui::RichText::new(reason.clone()).color(color));

                                    ui.weak(" — Unregister to free this slot");
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.inline_loading_indicator("registering asset");
                                    ui.weak("Waiting for the server to register it");
                                });
                            }
                        }
                    })
                },
                |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.take_available_height();
                        let more_response =
                            ui.small_icon_button(&re_ui::icons::MORE_VERTICAL, "more");

                        egui::Popup::menu(&more_response).show(|ui| {
                            asset_source_menu_ui(ui, ctx, asset);

                            if ui
                                .button(
                                    egui::RichText::new("Unregister asset")
                                        .color(ui.visuals().error_fg_color),
                                )
                                .clicked()
                            {
                                let origin = dataset.origin.clone();
                                let entry_id = dataset.id();
                                let asset_id = asset.id.clone();

                                // A failed asset holds no data, so it is unregistered right away.
                                // Unregistering a registered asset cannot be undone, so that one
                                // asks for confirmation first.
                                let command = if asset.has_failed() {
                                    Command::UnregisterAsset {
                                        origin,
                                        entry_id,
                                        asset_id,
                                        has_failed: true,
                                    }
                                } else {
                                    Command::OpenUnregisterAssetModal {
                                        origin,
                                        entry_id,
                                        asset_id,
                                    }
                                };

                                send_crossbeam(ctx.command_sender, command).ok();

                                ui.close();
                            }
                        });

                        // An asset that is not registered yet has nothing to open, so it only gets
                        // the menu.
                        if !asset.is_registered() {
                            return;
                        }

                        // `outlined` is the `ReButton` variant with a border.
                        let open_button = ui.add(re_ui::ReButton::new("Open").outlined().small());

                        if open_button.clicked_with_open_in_background() {
                            open_asset_ui(ui, dataset, asset, true);
                        } else if open_button.clicked() {
                            open_asset_ui(ui, dataset, asset, false);
                        }
                    });
                },
            );
    }
}

/// One term of a tab toolbar's metadata line, e.g. `12 segments`.
///
/// A term with no value yet shows a loading indicator in place of the number.
struct MetaTerm<'a> {
    value: Option<String>,
    label: &'a str,
}

/// The metadata line at the left end of a tab's toolbar, terms separated by a dot.
///
/// Every tab uses this, so the toolbars all look alike.
fn tab_meta_line_ui(ui: &mut egui::Ui, terms: &[MetaTerm<'_>]) {
    let label_color = ui.tokens().meta_line.label;
    let value_color = ui.tokens().meta_line.value;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = META_WORD_SPACE;

        for (index, MetaTerm { value, label }) in terms.iter().enumerate() {
            if index > 0 {
                ui.label(egui::RichText::new("·").color(label_color));
            }

            match value {
                Some(value) => {
                    ui.label(egui::RichText::new(value).color(value_color));
                }
                None => {
                    ui.inline_loading_indicator("waiting for the count");
                }
            }

            ui.label(egui::RichText::new(*label).color(label_color));
        }
    });
}

/// Opens an asset, from its card or from its "Open" button.
fn open_asset_ui(ui: &egui::Ui, dataset: &Dataset, asset: &Asset, new_tab: bool) {
    let url = re_viewer_context::open_url::ViewerOpenUrl::RedapDataset(re_uri::DatasetUri {
        origin: dataset.origin.clone(),
        dataset_id: dataset.id().id,
        resource: DatasetResource::Assets,
        segment_id: Some(asset.id.clone()),
        fragment: Default::default(),
    })
    .sharable_url(None);

    if let Ok(url) = url {
        ui.open_url(egui::OpenUrl { url, new_tab });
    }
}

/// Menu item opening [`crate::asset_source_modal::AssetSourceModal`] on an asset.
fn asset_source_menu_ui(ui: &mut egui::Ui, ctx: &Context<'_>, asset: &Asset) {
    if ui.button("Source URI details").clicked() {
        send_crossbeam(
            ctx.command_sender,
            Command::OpenAssetSourceModal {
                asset_id: asset.id.clone(),
                layers: asset.layers.clone(),
            },
        )
        .ok();

        ui.close();
    }
}

/// The line under an asset's name: its size, when it was registered, and when it was last updated
/// if that differs.
fn asset_meta_ui(ui: &mut egui::Ui, app_ctx: &AppContext<'_>, asset: &Asset) {
    ui.horizontal_wrapped(|ui| {
        let date_format = app_ctx.app_options.timestamp_format;
        ui.spacing_mut().item_spacing.x = 0.0;

        short_duration_ui(ui, asset.registered_at, date_format, |ui, registered_at| {
            ui.label(format!(
                "{} · Registered {registered_at}",
                re_format::format_bytes(asset.size as _)
            ))
        });

        // Registering an asset sets both timestamps, and the manifest rows of one asset can be
        // written a few milliseconds apart, so the two differ slightly even for an asset that was
        // never updated. Comparing the formatted text instead of the raw timestamps keeps the line
        // from showing the same moment twice. A re-registration is seconds or more later, see
        // `ConnectionHandle::register_asset`.
        if short_duration_text(asset.last_updated_at, date_format)
            != short_duration_text(asset.registered_at, date_format)
        {
            short_duration_ui(ui, asset.last_updated_at, date_format, |ui, updated_at| {
                ui.label(format!(" · Updated {updated_at}"))
            });
        }
    });
}

/// The height of one card in the asset list: a title with one line under it.
fn card_height(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Heading)
        + ui.spacing().item_spacing.y
        + ui.text_style_height(&egui::TextStyle::Body)
}

/// One card for a pending or failed registration.
fn asset_registration_ui(
    ui: &mut egui::Ui,
    ctx: &Context<'_>,
    dataset: &Dataset,
    registration: &AssetRegistration,
) {
    let color = registration_fg_color(&registration.state, ui.tokens());

    // A registration the server took is still pending as far as the user is concerned: the asset
    // shows up in the list only once the refetch behind it lands.
    let (pill_text, is_pending) = match &registration.state {
        RegistrationState::Pending | RegistrationState::Registered => ("pending", true),
        RegistrationState::Failed(_) => ("error", false),
    };

    egui::Sides::new()
        .shrink_left()
        .truncate()
        .height(card_height(ui))
        .show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    title_with_pill_ui(
                        ui,
                        source_uri_file_name(&registration.source_uri),
                        pill_text,
                        color,
                    )
                    .on_hover_text(&registration.source_uri);

                    match &registration.state {
                        RegistrationState::Pending | RegistrationState::Registered => {
                            ui.horizontal(|ui| {
                                ui.inline_loading_indicator("registering asset");
                                ui.weak("Waiting for the server to register it");
                            });
                        }

                        RegistrationState::Failed(err) => {
                            ui.label(egui::RichText::new(failure_reason(&err.error)).color(color));
                        }
                    }
                })
            },
            |ui| {
                ui.horizontal_centered(|ui| {
                    ui.take_available_height();

                    // Only the server can end a pending registration.
                    if !is_pending
                        && ui
                            .small_icon_button(&icons::CLOSE, "Dismiss")
                            .on_hover_text("Stop listing this registration")
                            .clicked()
                    {
                        send_crossbeam(
                            ctx.command_sender,
                            Command::DismissAssetRegistration {
                                origin: dataset.origin.clone(),
                                entry_id: dataset.id(),
                                source_uri: registration.source_uri.clone(),
                            },
                        )
                        .ok();
                    }
                });
            },
        );
}

fn registration_outline_color(state: &RegistrationState, tokens: &DesignTokens) -> egui::Color32 {
    match state {
        RegistrationState::Pending | RegistrationState::Registered => tokens.outlines.pending,
        RegistrationState::Failed(_) => tokens.outlines.error,
    }
}

fn registration_fg_color(state: &RegistrationState, tokens: &DesignTokens) -> egui::Color32 {
    match state {
        RegistrationState::Pending | RegistrationState::Registered => pending_fg_color(tokens),
        RegistrationState::Failed(_) => tokens.error_fg_color,
    }
}

/// What the server said went wrong.
///
/// `message` says which call failed, `source` says what went wrong with it. The whole error is in
/// the log.
fn failure_reason(err: &ApiError) -> String {
    err.source
        .as_ref()
        .map_or_else(|| err.message.clone(), |source| source.to_string())
}

/// The color a card is outlined with when the server has yet to finish registering its asset.
fn asset_outline_color(asset: &Asset, tokens: &DesignTokens) -> egui::Color32 {
    if asset.has_failed() {
        tokens.outlines.error
    } else {
        tokens.outlines.pending
    }
}

/// The color an asset is marked with when the server has yet to finish registering it.
fn asset_fg_color(asset: &Asset, tokens: &DesignTokens) -> egui::Color32 {
    if asset.has_failed() {
        tokens.error_fg_color
    } else {
        pending_fg_color(tokens)
    }
}

/// The color of a card that is waiting on the server.
fn pending_fg_color(tokens: &DesignTokens) -> egui::Color32 {
    tokens.warn_fg_color
}

/// The margin around the text of a pill.
const PILL_MARGIN: egui::Margin = egui::Margin::symmetric(6, 1);

/// How much of the outline color fills a pill, matching what the alert tokens use.
const PILL_FILL_ALPHA: u8 = 50;

/// The title of a card, with a pill after it saying what the card is waiting on.
fn title_with_pill_ui(
    ui: &mut egui::Ui,
    title: &str,
    pill_text: &str,
    color: egui::Color32,
) -> egui::Response {
    // A small rounded label tinted from the color the card is marked with.
    let pill_frame = egui::Frame::new()
        .fill(color.gamma_multiply_u8(PILL_FILL_ALPHA))
        .corner_radius(u8::MAX)
        .inner_margin(PILL_MARGIN);

    let pill = egui::AtomLayout::new(egui::RichText::new(pill_text).monospace().color(color))
        .frame(pill_frame);

    // The title gives up the room the pill needs, since the pill is squeezed out of the row
    // otherwise.
    ui.add(egui::AtomLayout::new((
        egui::RichText::new(title)
            .heading()
            .strong()
            .atom_shrink(true),
        pill,
    )))
}

/// The file name at the end of the uri an asset is registered from.
///
/// A card has room for the file name but not for the whole uri, and the file name is what tells one
/// registration from another.
fn source_uri_file_name(source_uri: &str) -> &str {
    let path = source_uri
        .split_once(['?', '#'])
        .map_or(source_uri, |(path, _)| path);

    match path.trim_end_matches('/').rsplit('/').next() {
        Some(file_name) if !file_name.is_empty() => file_name,
        _ => source_uri,
    }
}

/// The assets tab of a dataset that has none: what an asset is, and how to register one.
fn no_assets_ui(
    ui: &mut egui::Ui,
    ctx: &Context<'_>,
    asset_target: &AssetTarget,
    asset_slots: &AssetSlots,
) {
    /// The explanation wraps within this width, so it stays readable on a wide screen.
    const EXPLANATION_WIDTH: f32 = 460.0;

    /// The rounded square behind the icon.
    const ICON_BOX_SIZE: f32 = 44.0;

    const ICON_SIZE: f32 = 20.0;

    const DOC_URL: &str =
        "https://rerun.io/docs/concepts/query-and-transform/catalog-object-model#assets";

    let tokens = ui.tokens();

    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(
            tokens.table_grid_view_card_inner_margin as i8,
            48,
        ))
        // The same surface as a card, so the empty state looks like the cards it replaces. It is a
        // plain frame rather than a `CardLayout` card, so it never reacts to hover or clicks. Only
        // the buttons inside it are interactive.
        .fill(tokens.card_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(tokens.table_grid_view_card_corner_radius)
        .show(ui, |ui| {
            ui.take_available_width();

            ui.vertical_centered(|ui| {
                let (_, box_rect) = ui.allocate_space(egui::Vec2::splat(ICON_BOX_SIZE));
                ui.painter()
                    .rect_filled(box_rect, 8.0, tokens.faint_bg_color);
                icons::ASSET.as_image().tint(tokens.text_subdued).paint_at(
                    ui,
                    egui::Rect::from_center_size(box_rect.center(), egui::Vec2::splat(ICON_SIZE)),
                );

                ui.add_space(12.0);

                ui.label(
                    egui::RichText::new("No assets registered")
                        .heading()
                        .strong(),
                );

                ui.add_space(6.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(EXPLANATION_WIDTH, 0.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(
                                    "Assets are files shared by the segments of this dataset — a \
                                     robot URDF, a room mesh, a calibration. Register one once \
                                     instead of copying it into every recording.",
                                )
                                .color(tokens.text_subdued),
                            )
                            .halign(egui::Align::Center)
                            .wrap(),
                        );
                    },
                );

                ui.add_space(16.0);

                centered_row(ui, "asset limits", |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    for rule in asset_slots.limit_rules() {
                        egui::Frame::new()
                            .fill(tokens.faint_bg_color)
                            .corner_radius(u8::MAX)
                            .inner_margin(egui::Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(rule)
                                        .monospace()
                                        .color(tokens.text_subdued),
                                );
                            });
                    }
                });

                ui.add_space(16.0);

                centered_row(ui, "asset actions", |ui| {
                    register_asset_button(ctx, asset_target, asset_slots, ui);

                    let link_color = tokens.button_blue.fill;
                    if ui
                        .add(
                            ReButton::new((
                                egui::RichText::new("Assets in doc").color(link_color),
                                icons::EXTERNAL_LINK.as_image().tint(link_color),
                            ))
                            .ghost()
                            .small()
                            .image_tint_follows_text_color(false),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        ui.open_url(egui::OpenUrl::new_tab(DOC_URL));
                    }
                });

                ui.add_space(16.0);

                ui.label(
                    egui::RichText::new("dataset.register_asset(\"s3://path/to/asset.rrd\")")
                        .monospace()
                        .weak(),
                );
            });
        });
}

/// Shows a row of widgets, centered in the available width.
///
/// How wide the row is only shows once it has been laid out, so the pass that lays it out for the
/// first time is discarded and the row is centered in the next one.
// TODO(isse): Use https://github.com/emilk/egui/pull/8216 once it lands.
fn centered_row<R>(
    ui: &mut egui::Ui,
    id_salt: impl egui::AsIdSalt,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let id = ui.make_persistent_id(id_salt);
    let row_width: Option<f32> = ui.data(|data| data.get_temp(id));

    if row_width.is_none() {
        ui.ctx().request_discard("centered row without a width yet");
    }

    let mut leading_space = 0.0;

    let response = ui.horizontal(|ui| {
        if let Some(row_width) = row_width {
            leading_space = ((ui.available_width() - row_width) / 2.0).max(0.0);
            ui.add_space(leading_space);
        }

        add_contents(ui)
    });

    ui.data_mut(|data| data.insert_temp(id, response.response.rect.width() - leading_space));

    response.inner
}

/// Button that opens the register modal, disabled while the dataset has no room for another asset.
fn register_asset_button(
    ctx: &Context<'_>,
    asset_target: &AssetTarget,
    asset_slots: &AssetSlots,
    ui: &mut egui::Ui,
) {
    let no_room_reason = asset_slots.no_room_reason();

    let response = ui.add_enabled(
        no_room_reason.is_none(),
        ReButton::new("Register asset").blue().small(),
    );

    if let Some(reason) = no_room_reason {
        response.on_disabled_hover_text(reason);
    } else if response.clicked() {
        send_crossbeam(
            ctx.command_sender,
            Command::OpenRegisterAssetModal(asset_target.clone()),
        )
        .ok();
    }
}

/// Unregisters an asset of a dataset.
///
/// An asset whose registration failed is only dropped by force, while one the server is done with
/// is dropped on its own terms, so a layer the server is still working on stays.
///
/// Returns whether the server dropped it.
pub(crate) async fn unregister_asset(
    connection: ConnectionHandle,
    dataset_id: EntryId,
    asset_id: SegmentId,
    has_failed: bool,
) -> bool {
    match connection
        .unregister_asset(
            dataset_id,
            asset_id.clone(),
            has_failed,
            DEFAULT_ASSET_TASK_TIMEOUT,
        )
        .await
    {
        Ok(()) => {
            re_log::info!("Successfully unregistered asset '{asset_id}'");
            true
        }
        Err(err) => {
            re_log::error!("Failed unregistering asset '{asset_id}': {err}");
            false
        }
    }
}

/// Button that refetches the dataset from the server.
///
/// Sits next to the dataset's name, and on hover shows how long ago the dataset was fetched.
/// `queried_at` is when the client last fetched it, or `None` before the first fetch.
fn refresh_button_ui(
    ui: &mut egui::Ui,
    app_ctx: &AppContext<'_>,
    dataset: &Dataset,
    queried_at: Option<Timestamp>,
) {
    let shortcut = TableCommandKind::Refresh.formatted_kb_shortcut(ui.ctx());
    let timestamp_format = app_ctx.app_options.timestamp_format;

    if ui
        .small_icon_button(&icons::RESET, "Refresh dataset")
        // Only built while hovered, so the duration below is formatted, and requests repaints,
        // only while the tooltip is on screen.
        .on_hover_ui(|ui| {
            match &shortcut {
                Some(shortcut) => ui.label(format!("Refresh dataset ({shortcut})")),
                None => ui.label("Refresh dataset"),
            };

            // Worded and styled like the segment table's bottom bar, which shows the same
            // timestamp for the same table.
            if let Some(queried_at) = queried_at {
                ui.horizontal(|ui| {
                    ui.label("Last updated:");
                    short_duration_ui(ui, queried_at, timestamp_format, egui::Ui::strong);
                });
            }
        })
        .clicked()
    {
        app_ctx.command_sender().send_table_command(TableCommand {
            origin: dataset.origin.clone(),
            entry_id: dataset.id(),
            kind: TableCommandKind::Refresh,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::source_uri_file_name;

    /// A registration card is titled with the file name of the uri it registers, whatever scheme
    /// the uri uses and whatever query it carries. A uri with nothing to take a file name from
    /// keeps the whole uri as its title.
    #[test]
    fn source_uri_file_name_takes_the_last_part_of_the_uri() {
        assert_eq!(
            source_uri_file_name("file:///recordings/data.rrd"),
            "data.rrd"
        );
        assert_eq!(
            source_uri_file_name("s3://a-bucket/assets/robot_urdf.rrd"),
            "robot_urdf.rrd"
        );
        assert_eq!(
            source_uri_file_name("https://example.com/assets/robot_urdf.rrd?token=secret#layer"),
            "robot_urdf.rrd"
        );
        assert_eq!(
            source_uri_file_name("https://example.com/assets/"),
            "assets"
        );
        assert_eq!(source_uri_file_name("robot_urdf.rrd"), "robot_urdf.rrd");
        assert_eq!(source_uri_file_name(""), "");
    }
}
