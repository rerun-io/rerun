use re_viewer_context::Item;

use crate::misc::space_info::SpaceInfoCollection;
use re_arrow_store::{TimeInt, Timeline};
use re_log_types::Component;
use re_viewer_context::ViewerContext;

use crate::blueprint_components::PanelState;

use super::viewport::Viewport;

/// Defines the layout of the whole Viewer (or will, eventually).
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Blueprint {
    pub blueprint_panel_expanded: bool,
    pub selection_panel_expanded: bool,
    pub time_panel_expanded: bool,

    pub viewport: Viewport,
}

impl Blueprint {
    pub fn from_db(egui_ctx: &egui::Context, blueprint_db: &re_data_store::LogDb) -> Self {
        let mut ret = Self::new(egui_ctx);

        // TODO(jleibs): This is going to need to be a LOT more ergonomic
        let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);

        let blueprint_state = blueprint_db.entity_db.data_store.latest_at(
            &query,
            &PanelState::BLUEPRINT_PANEL.into(),
            PanelState::name(),
            &[PanelState::name()],
        );
        ret.blueprint_panel_expanded = blueprint_state.map_or(true, |(_, data)| {
            data[0].as_ref().map_or(true, |cell| {
                cell.try_to_native::<PanelState>()
                    .unwrap()
                    .next()
                    .unwrap()
                    .expanded
            })
        });

        let selection_state = blueprint_db.entity_db.data_store.latest_at(
            &query,
            &PanelState::SELECTION_PANEL.into(),
            PanelState::name(),
            &[PanelState::name()],
        );
        ret.selection_panel_expanded = selection_state.map_or(true, |(_, data)| {
            data[0].as_ref().map_or(true, |cell| {
                cell.try_to_native::<PanelState>()
                    .unwrap()
                    .next()
                    .unwrap()
                    .expanded
            })
        });

        let timeline_state = blueprint_db.entity_db.data_store.latest_at(
            &query,
            &PanelState::TIMELINE_PANEL.into(),
            PanelState::name(),
            &[PanelState::name()],
        );
        ret.time_panel_expanded = timeline_state.map_or(true, |(_, data)| {
            data[0].as_ref().map_or(true, |cell| {
                cell.try_to_native::<PanelState>()
                    .unwrap()
                    .next()
                    .unwrap()
                    .expanded
            })
        });
        ret
    }

    pub fn process_updates(&self, snapshot: &Self) {
        if self.blueprint_panel_expanded != snapshot.blueprint_panel_expanded {
            re_log::info!(
                "blueprint_panel_expanded: {}",
                self.blueprint_panel_expanded
            );
        }
        if self.selection_panel_expanded != snapshot.selection_panel_expanded {
            re_log::info!(
                "selection_panel_expanded: {}",
                self.selection_panel_expanded
            );
        }
        if self.time_panel_expanded != snapshot.time_panel_expanded {
            re_log::info!("time_panel_expanded: {}", self.time_panel_expanded);
        }
    }

    /// Prefer this to [`Blueprint::default`] to get better defaults based on screen size.
    pub fn new(egui_ctx: &egui::Context) -> Self {
        let screen_size = egui_ctx.screen_rect().size();
        Self {
            blueprint_panel_expanded: screen_size.x > 750.0,
            selection_panel_expanded: screen_size.x > 1000.0,
            time_panel_expanded: screen_size.y > 600.0,
            viewport: Default::default(),
        }
    }

    pub fn blueprint_panel_and_viewport(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        let spaces_info = SpaceInfoCollection::new(&ctx.log_db.entity_db);

        self.viewport.on_frame_start(ctx, &spaces_info);

        self.blueprint_panel(ctx, ui, &spaces_info);

        let viewport_frame = egui::Frame {
            fill: ui.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(viewport_frame)
            .show_inside(ui, |ui| {
                self.viewport.viewport_ui(ui, ctx);
            });
    }

    fn blueprint_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
    ) {
        let screen_width = ui.ctx().screen_rect().width();

        let panel = egui::SidePanel::left("blueprint_panel")
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.visuals().panel_fill,
                ..Default::default()
            })
            .min_width(120.0)
            .default_width((0.35 * screen_width).min(200.0).round());

        panel.show_animated_inside(ui, self.blueprint_panel_expanded, |ui: &mut egui::Ui| {
            self.title_bar_ui(ctx, ui, spaces_info);

            egui::Frame {
                inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                ..Default::default()
            }
            .show(ui, |ui| {
                self.viewport.tree_ui(ctx, ui);
            });
        });
    }

    fn title_bar_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
    ) {
        egui::TopBottomPanel::top("blueprint_panel_title_bar")
            .exact_height(re_ui::ReUi::title_bar_height())
            .frame(egui::Frame {
                inner_margin: egui::Margin::symmetric(re_ui::ReUi::view_padding(), 0.0),
                ..Default::default()
            })
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.strong("Blueprint").on_hover_text(
                        "The Blueprint is where you can configure the Rerun Viewer.",
                    );

                    ui.allocate_ui_with_layout(
                        ui.available_size_before_wrap(),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            self.viewport
                                .add_new_spaceview_button_ui(ctx, ui, spaces_info);
                            self.reset_button_ui(ctx, ui, spaces_info);
                        },
                    );
                });
            });
    }

    fn reset_button_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
    ) {
        if ctx
            .re_ui
            .small_icon_button(ui, &re_ui::icons::RESET)
            .on_hover_text("Re-populate Viewport with automatically chosen Space Views")
            .clicked()
        {
            self.viewport = Viewport::new(ctx, spaces_info);
        }
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    pub fn is_item_valid(&self, item: &Item) -> bool {
        match item {
            Item::ComponentPath(_) => true,
            Item::InstancePath(space_view_id, _) => space_view_id
                .map(|space_view_id| self.viewport.space_view(&space_view_id).is_some())
                .unwrap_or(true),
            Item::SpaceView(space_view_id) => self.viewport.space_view(space_view_id).is_some(),
            Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
                if let Some(space_view) = self.viewport.space_view(space_view_id) {
                    space_view
                        .data_blueprint
                        .group(*data_blueprint_group_handle)
                        .is_some()
                } else {
                    false
                }
            }
        }
    }
}
