use super::Blueprint;
use crate::ViewerContext;
use egui::Vec2;
use egui_dock::{DockArea, TabViewer, Tree};

struct Tabs<'a, 'b> {
    ctx: &'a mut ViewerContext<'b>,
}

impl<'a, 'b> Tabs<'a, 'b> {
    fn xlink_statistics_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Xlink");
    }

    fn imu_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("IMU");
    }
}

impl<'a, 'b> TabViewer for Tabs<'a, 'b> {
    type Tab = Tab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::XlinkStatistics => self.xlink_statistics_ui(ui),
            Tab::Imu => self.imu_ui(ui),
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            Tab::XlinkStatistics => "Xlink Statistics".into(),
            Tab::Imu => "IMU".into(),
        }
    }
}

enum Tab {
    XlinkStatistics,
    Imu,
}

/// The bottom panel of the viewer.
/// This is where XlinkOut statistics and IMU are logged.
/// In the future this panel will also be used to replay recordings. (that will likely look mostly like time_panel)
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BottomPanel {
    #[serde(skip)]
    dock_tree: Tree<Tab>,
}

impl Default for BottomPanel {
    fn default() -> Self {
        Self {
            dock_tree: Tree::new(vec![Tab::XlinkStatistics, Tab::Imu]),
        }
    }
}

impl BottomPanel {
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        ui: &mut egui::Ui,
    ) {
        let top_bar_height = 28.0;
        let margin = ctx.re_ui.bottom_panel_margin();
        let mut panel_frame = ctx.re_ui.bottom_panel_frame();

        let screen_height = ui.ctx().screen_rect().width();

        let collapsed = egui::TopBottomPanel::bottom("bottom_panel_collapsed")
            .resizable(false)
            .show_separator_line(false)
            .frame(panel_frame)
            .default_height(44.0);

        let min_height = 150.0;
        let expanded = egui::TopBottomPanel::bottom("bottom_panel_expanded")
            .resizable(true)
            .show_separator_line(false)
            .frame(panel_frame)
            .min_height(min_height)
            .default_height((0.25 * screen_height).clamp(min_height, 250.0).round());

        egui::TopBottomPanel::show_animated_between_inside(
            ui,
            true,
            collapsed,
            expanded,
            |ui: &mut egui::Ui, expansion: f32| {
                if expansion < 1.0 {
                    // Collapsed or animating
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size = Vec2::splat(top_bar_height);
                        ui.visuals_mut().button_frame = true;
                        // self.collapsed_ui(ctx, ui);
                    });
                } else {
                    // Expanded:
                        // Add extra margin on the left which was intentionally missing on the controls.
                        let mut top_rop_frame = egui::Frame::default();
                        top_rop_frame.inner_margin.left = 8.0;
                        top_rop_frame.show(ui, |ui| {
                            DockArea::new(&mut self.dock_tree)
                                .id(egui::Id::new("bottom_panel_tabs"))
                                .style(re_ui::egui_dock_style(ui.style()))
                                .show_inside(ui, &mut Tabs { ctx });
                        });
                }
            },
        );
    }
}
