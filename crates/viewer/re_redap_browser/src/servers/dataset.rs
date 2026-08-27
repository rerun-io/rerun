use datafusion::sql::TableReference;
use re_dataframe_ui::{ColumnBlueprint, default_display_name_for_column};
use re_format::format_plural_s;
use re_log_types::EntityPathPart;
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_redap_client::{ApiError, Asset};
use re_ui::egui_ext::card_layout::CardLayout;
use re_ui::time::short_duration_ui;
use re_ui::{
    ServerValue, TabBar, TableCommand, TableCommandKind, TableCommandSender as _, UiExt as _, icons,
};
use re_uri::DatasetResource;
use re_viewer_context::{
    AppContext, SystemCommand, SystemCommandSender as _, TableReference as ViewerTableReference,
    ViewStates,
};

use crate::entry_meta::{AssetsRef, EntryMeta, EntryMetaQuery};
use crate::{Server, entries::Dataset};

const PADDING: f32 = 16.0;

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
            let name = dataset.name();

            let path = name
                .as_str()
                .split(re_uri::DATASET_HIERARCHY_SEPARATOR)
                .collect::<Vec<_>>();

            ui.horizontal(|ui| {
                ui.add_space(PADDING);
                ui.vertical(|ui| {
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
                    }

                    if let Some(name) = path.last() {
                        ui.label(egui::RichText::new(*name).heading().strong());
                    }

                    ui.horizontal(|ui| {
                        self.entry_meta_ui(ui, dataset);
                        refresh_button_ui(ui, app_ctx, dataset);
                    });
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
                    self.assets_ui(ui, app_ctx, dataset);
                }
            }
        });
    }

    /// The line under a dataset's name, summarizing what the server says about it.
    fn entry_meta_ui(&self, ui: &mut egui::Ui, dataset: &Dataset) {
        let EntryMeta { columns } = dataset
            .requests()
            .meta(self.entry_meta_query(ui.ctx(), dataset));

        if let Some(&columns) = columns.get() {
            ui.weak(format_plural_s(columns, "column"));
        } else {
            ui.inline_loading_indicator("waiting for schema");
        }
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
        .column_blueprint(|desc| {
            let mut name = default_display_name_for_column(desc);

            // strip prefix and remove underscores, _only_ for the base columns (aka not the
            // properties)
            name = name
                .strip_prefix("rerun_")
                .map(|name| name.replace('_', " "))
                .unwrap_or(name);

            let default_visible = if desc.entity_path().is_some_and(|entity_path| {
                entity_path.starts_with(&std::iter::once(EntityPathPart::properties()).collect())
            }) {
                // Property columns are visible by default
                true
            } else {
                desc.display_name().as_str() == RECORDING_LINK_COLUMN_NAME
            };

            let column_sort_key = match desc.display_name().as_str() {
                ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME => 0,
                RECORDING_LINK_COLUMN_NAME => 1,
                _ => 2,
            };

            let mut blueprint = ColumnBlueprint::default()
                .display_name(name)
                .default_visibility(default_visible)
                .sort_key(column_sort_key);

            if desc.display_name().as_str() == RECORDING_LINK_COLUMN_NAME {
                blueprint = blueprint.variant_ui(re_component_ui::REDAP_URI_BUTTON_VARIANT);
            }

            blueprint
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
    fn assets_ui(&self, ui: &mut egui::Ui, ctx: &AppContext<'_>, dataset: &Dataset) {
        let assets = dataset.requests().assets(
            self.entry_meta_query(ui.ctx(), dataset),
            dataset.asset_dataset(),
        );

        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(PADDING as i8, 0))
            .show(ui, |ui| asset_card_list(ui, ctx, dataset, &assets));
    }
}

/// Button that refetches the dataset from the server.
fn refresh_button_ui(ui: &mut egui::Ui, app_ctx: &AppContext<'_>, dataset: &Dataset) {
    let tooltip = match TableCommandKind::Refresh.formatted_kb_shortcut(ui.ctx()) {
        Some(shortcut) => format!("Refresh dataset ({shortcut})"),
        None => "Refresh dataset".to_owned(),
    };

    if ui
        .small_icon_button(&icons::RESET, "Refresh dataset")
        .on_hover_text(tooltip)
        .clicked()
    {
        app_ctx.command_sender().send_table_command(TableCommand {
            origin: dataset.origin.clone(),
            entry_id: dataset.id(),
            kind: TableCommandKind::Refresh,
        });
    }
}

fn asset_card_list(
    ui: &mut egui::Ui,
    ctx: &AppContext<'_>,
    dataset: &Dataset,
    assets: &ServerValue<AssetsRef, ApiError>,
) {
    let tokens = ui.tokens();
    ui.horizontal(|ui| {
        let len = assets.get().map(|a| a.len().to_string());
        egui::Frame::new()
            .fill(tokens.faint_bg_color)
            .corner_radius(u8::MAX)
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.label(format!("{} of {}", len.as_deref().unwrap_or("?"), 12));
            });

        let total_bytes = assets
            .get()
            .map(|a| a.iter().map(|a| a.size as f64).sum::<f64>())
            .map(re_format::format_bytes);
        ui.weak(format!("{} total", total_bytes.as_deref().unwrap_or("?")));

        if matches!(assets, ServerValue::Pending { .. }) {
            ui.inline_loading_indicator("asset list pending");
        }

        if let Some(err) = assets.get_err() {
            ui.error_label(err.to_string());
        }
    });

    let Some(assets) = assets.get() else {
        return;
    };

    let inner_margin = egui::Margin::same(tokens.table_grid_view_card_inner_margin as i8);
    let card_frame = egui::Frame::new()
        .inner_margin(inner_margin)
        .fill(tokens.extreme_bg_color)
        .stroke(egui::Stroke::new(1.0, tokens.faint_bg_color))
        .corner_radius(tokens.table_grid_view_card_corner_radius);

    egui::ScrollArea::vertical().show(ui, |ui| {
        let old_item_spacing =
            std::mem::replace(&mut ui.spacing_mut().item_spacing, egui::vec2(0.0, 11.0));

        CardLayout::uniform(assets.len(), ui.available_width(), card_frame).show(
            ui,
            |ui, idx, _hovered| {
                ui.spacing_mut().item_spacing = old_item_spacing;
                let Some(asset) = assets.get(idx) else {
                    return;
                };

                asset_ui(ui, ctx, dataset, asset);
            },
        );
    });

    ui.weak("static data only · ≤ 300 MiB · 12 assets per dataset");
}

fn asset_ui(ui: &mut egui::Ui, ctx: &AppContext<'_>, dataset: &Dataset, asset: &Asset) {
    // Both sides stack two lines of text, and `Sides` centers its content within the height it
    // is given.
    let row_height = ui
        .text_style_height(&egui::TextStyle::Heading)
        .max(ui.text_style_height(&egui::TextStyle::Monospace))
        + ui.spacing().item_spacing.y
        + ui.text_style_height(&egui::TextStyle::Body);

    egui::Sides::new().shrink_left().height(row_height).show(
        ui,
        |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(asset.id.as_str()).heading().strong());
                ui.horizontal(|ui| {
                    let date_format = ctx.app_options.timestamp_format;
                    ui.spacing_mut().item_spacing.x = 0.0;

                    short_duration_ui(ui, asset.registered_at, date_format, |ui, registered_at| {
                        short_duration_ui(
                            ui,
                            asset.last_updated_at,
                            date_format,
                            |ui, updated_at| {
                                ui.label(format!(
                                    "Registered {registered_at}  · Updated {updated_at}"
                                ))
                            },
                        )
                    });
                });
            })
        },
        |ui| {
            ui.horizontal_centered(|ui| {
                ui.take_available_height();
                let add_button = ui.add(
                    re_ui::ReButton::from_button(egui::Button::new("Open"))
                        .ghost()
                        .stroke(egui::Stroke::new(1.0, ui.tokens().faint_bg_color)),
                );

                ui.add_space(4.0);

                ui.separator();

                ui.add_space(8.0);

                if let Ok(url) =
                    re_viewer_context::open_url::ViewerOpenUrl::RedapDataset(re_uri::DatasetUri {
                        origin: dataset.origin.clone(),
                        dataset_id: dataset.id().id,
                        resource: re_uri::DatasetResource::Assets,
                        segment_id: Some(asset.id.clone()),
                        fragment: Default::default(),
                    })
                    .sharable_url(None)
                {
                    if add_button.clicked() {
                        ui.open_url(egui::OpenUrl {
                            url,
                            new_tab: false,
                        });
                    } else if add_button.middle_clicked() {
                        ui.open_url(egui::OpenUrl { url, new_tab: true });
                    }
                }

                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    ui.label(egui::RichText::new("SIZE").weak().monospace());
                    ui.label(
                        egui::RichText::new(re_format::format_bytes(asset.size as _))
                            .strong()
                            .monospace()
                            .size(13.0),
                    );
                });
            });
        },
    );
}
