use crate::misc::{space_info::SpacesInfo, ViewerContext};

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

        let spaces_info = SpacesInfo::new(&ctx.log_db.obj_db, &ctx.rec_cfg.time_ctrl);

        self.viewport.on_frame_start(ctx, &spaces_info);

        self.blueprint_panel(ctx, ui, &spaces_info);

        let viewport_frame = egui::Frame {
            fill: ui.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(viewport_frame)
            .show_inside(ui, |ui| {
                self.viewport.viewport_ui(
                    ui,
                    ctx,
                    &spaces_info,
                    &mut self.selection_panel_expanded,
                );
            });
    }

    fn blueprint_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
    ) {
        let panel = egui::SidePanel::left("blueprint_panel")
            .resizable(true)
            .frame(ctx.re_ui.panel_frame())
            .min_width(120.0)
            .default_width(200.0);

        panel.show_animated_inside(ui, self.blueprint_panel_expanded, |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                ui.strong("Blueprint")
                    .on_hover_text("The Blueprint is where you can configure the Rerun Viewer.");

                // TODO(emilk): an egui helper for right-to-left
                ui.allocate_ui_with_layout(
                    egui::vec2(
                        ui.available_size_before_wrap().x,
                        ui.spacing().interact_size.y,
                    ),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        self.viewport
                            .add_new_spaceview_button_ui(ctx, ui, spaces_info);
                        self.reset_button_ui(ctx, ui, spaces_info);
                    },
                );
            });

            ui.separator();

            self.viewport.tree_ui(ctx, ui);
        });
    }

    fn reset_button_ui(
        &mut self,
        ctx: &mut ViewerContext,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
    ) {
        if ctx
            .re_ui
            .small_icon(ui, &re_ui::icons::RESET)
            .on_hover_text("Re-populate Viewport with automatically chosen Space Views")
            .clicked()
        {
            self.viewport = Viewport::new(ctx, spaces_info);
        }
    }
}
