use crate::misc::{space_info::SpaceInfoCollection, ViewerContext};

use super::viewport::Viewport;

/// Defines the layout of the whole Viewer (or will, eventually).
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Blueprint {
    pub blueprint_panel_expanded: bool,
    pub selection_panel_expanded: bool,
    pub time_panel_expanded: bool,

    pub viewport: Viewport,
}

impl Default for Blueprint {
    fn default() -> Self {
        Self {
            blueprint_panel_expanded: true,
            selection_panel_expanded: true,
            time_panel_expanded: true,
            viewport: Default::default(),
        }
    }
}

impl Blueprint {
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
                self.viewport.viewport_ui(ui, ctx, &spaces_info);
            });
    }

    fn blueprint_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpaceInfoCollection,
    ) {
        let panel = egui::SidePanel::left("blueprint_panel")
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.visuals().panel_fill,
                ..Default::default()
            })
            .min_width(120.0)
            .default_width(200.0);

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
}
