use egui::{
    emath::History,
    plot::{Line, Plot, PlotPoints},
};
use egui_dock::{TabViewer, Tree};
use itertools::Itertools;
use re_arrow_store::{LatestAtQuery, TimeInt, Timeline};
use re_log_types::{
    component_types::{ImuData, XlinkStats},
    Component,
};
use strum::{EnumIter, IntoEnumIterator};

use crate::{depthai::depthai, misc::ViewerContext};

use super::Viewport;

pub struct StatsPanel {}

impl StatsPanel {
    pub fn show_panel(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, state: &mut StatsPanelState) {
        let mut tree = state.tree.clone(); // Have to clone to avoid borrowing issue
        state.imu_tab_visible = false; // Check every frame if the IMU tab is visible
        egui_dock::DockArea::new(&mut tree)
            .id(egui::Id::new("stats_panel"))
            .style(re_ui::egui_dock_style(ui.style()))
            .show_inside(ui, &mut StatsTabs { ctx, state });
        state.tree = tree;
    }

    pub fn stats_panel_options_ui(
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        viewport: &mut Viewport,
        tab_bar_rect: egui::Rect,
    ) {
    }
}

#[derive(Debug, Copy, Clone, EnumIter, PartialEq, Eq)]
enum ImuTabKind {
    Accel,
    Gyro,
    Mag,
}

#[derive(Debug, Copy, Clone, EnumIter, PartialEq, Eq)]
enum Xyz {
    X,
    Y,
    Z,
}

#[derive(Debug, Copy, Clone, EnumIter, PartialEq, Eq)]
pub enum StatTabKind {
    Imu,
    Xlink,
}

pub struct StatsPanelState {
    tree: Tree<StatTabKind>,
    accel_history: History<[f32; 3]>,
    gyro_history: History<[f32; 3]>,
    magnetometer_history: History<[f32; 3]>,
    start_time: instant::Instant, // Time elapsed from spawning the app
    imu_tab_visible: bool,        // Used to subscribe and unsubscribe from the IMU data
    xlink_stats_history: History<[f64; 4]>, // [MB written in last time frame, MB read in last time frame, MB written total, MB read total]
    avg_xlink_stats_plot_history: History<[f64; 2]>, // [Avg MB written, Avg MB read]
}

impl Default for StatsPanelState {
    fn default() -> Self {
        Self {
            tree: Tree::new(StatTabKind::iter().collect_vec()),
            accel_history: History::new(0..1000, 5.0),
            gyro_history: History::new(0..1000, 5.0),
            magnetometer_history: History::new(0..1000, 5.0),
            start_time: instant::Instant::now(),
            imu_tab_visible: false,
            xlink_stats_history: History::new(0..1000, 1.0),
            avg_xlink_stats_plot_history: History::new(0..1000, 5.0),
        }
    }
}

impl StatsPanelState {
    pub fn update(&mut self, ctx: &mut ViewerContext<'_>) {
        self.update_imu(ctx);
        self.update_xlink(ctx);
    }
    /// Push new data into the history buffers.
    fn update_imu(&mut self, ctx: &mut ViewerContext<'_>) {
        self.update_imu_subscription(ctx);
        let now = self.start_time.elapsed().as_secs_f64();
        let imu_entity_path = &ImuData::entity_path();
        if let Ok(latest) = re_query::query_entity_with_primary::<ImuData>(
            &ctx.log_db.entity_db.data_store,
            &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
            imu_entity_path,
            &[ImuData::name()],
        ) {
            let _ = latest.visit1(|_inst, imu_data| {
                self.accel_history
                    .add(now, [imu_data.accel.x, imu_data.accel.y, imu_data.accel.z]);
                self.gyro_history
                    .add(now, [imu_data.gyro.x, imu_data.gyro.y, imu_data.gyro.z]);
                if let Some(mag) = imu_data.mag {
                    self.magnetometer_history.add(now, [mag.x, mag.y, mag.z]);
                }
            });
        }
    }

    fn update_xlink(&mut self, ctx: &mut ViewerContext<'_>) {
        let now = self.start_time.elapsed().as_secs_f64();
        let entity_path = &XlinkStats::entity_path();
        if let Ok(latest) = re_query::query_entity_with_primary::<XlinkStats>(
            &ctx.log_db.entity_db.data_store,
            &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
            entity_path,
            &[XlinkStats::name()],
        ) {
            let _ = latest.visit1(|_inst, xlink_stats| {
                let (mut written, mut read) = (
                    (xlink_stats.bytes_written / 1e6 as i64) as f64,
                    (xlink_stats.bytes_read / 1e6 as i64) as f64,
                );
                if let Some((time, [_, _, total_written, total_read])) =
                    self.xlink_stats_history.iter().last()
                {
                    written = (written - total_written) / (now - time);
                    read = (read - total_read) / (now - time);
                }

                self.xlink_stats_history.add(
                    now,
                    [
                        written,
                        read,
                        (xlink_stats.bytes_written / 1e6 as i64) as f64,
                        (xlink_stats.bytes_read / 1e6 as i64) as f64,
                    ],
                );
                self.avg_xlink_stats_plot_history.add(
                    now,
                    [
                        self.xlink_stats_history
                            .iter()
                            .map(|(_, [written, _, _, _])| written)
                            .sum::<f64>()
                            / self.xlink_stats_history.len() as f64,
                        self.xlink_stats_history
                            .iter()
                            .map(|(_, [_, read, _, _])| read)
                            .sum::<f64>()
                            / self.xlink_stats_history.len() as f64,
                    ],
                );
            });
        }
    }

    pub fn update_imu_subscription(&mut self, ctx: &mut ViewerContext<'_>) {
        let unsub = !self.imu_tab_visible
            && ctx
                .depthai_state
                .subscriptions
                .contains(&depthai::ChannelId::ImuData);
        if unsub {
            let subs = ctx
                .depthai_state
                .subscriptions
                .iter()
                .filter_map(|x| {
                    if x != &depthai::ChannelId::ImuData {
                        return Some(x.clone());
                    } else {
                        return None;
                    }
                })
                .collect_vec();
            ctx.depthai_state.set_subscriptions(&subs);
            self.accel_history.clear();
            self.gyro_history.clear();
            self.magnetometer_history.clear();
        } else if self.imu_tab_visible
            && !ctx
                .depthai_state
                .subscriptions
                .contains(&depthai::ChannelId::ImuData)
        {
            let mut subs = ctx.depthai_state.subscriptions.clone();
            subs.push(depthai::ChannelId::ImuData);
            ctx.depthai_state.set_subscriptions(&subs);
        }
    }
}

struct StatsTabs<'a, 'b> {
    ctx: &'a mut ViewerContext<'b>,
    state: &'a mut StatsPanelState,
}

impl<'a, 'b> StatsTabs<'a, 'b> {
    fn imu_ui(&mut self, ui: &mut egui::Ui) {
        let imu_entity_path = &ImuData::entity_path();
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Frame {
                inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                ..Default::default()
            }
            .show(ui, |ui| {
                let max_width = ui.available_width();
                for kind in ImuTabKind::iter() {
                    self.xyz_plot_ui(ui, kind, max_width);
                }
            });
        });
    }

    fn xlink_ui(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::Frame {
                inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                ..Default::default()
            }
            .show(ui, |ui| {
                let (history, display_name, unit) = (
                    &mut self.state.avg_xlink_stats_plot_history,
                    "XLink throughput",
                    "",
                );
                let Some(latest) = history.latest() else {
                ui.label(format!("No {display_name} data yet"));
                return;
            };
                ui.label(format!(
                    "{display_name}: avg. Sent from device {:.2} MB/s, avg. Sent to Device: {:.2} MB/s",
                    latest[0], latest[1]
                ));
                Plot::new(display_name).show(ui, |plot_ui| {
                    plot_ui.line(
                        Line::new(PlotPoints::new(
                            history
                                .iter()
                                .map(|(t, [written, _])| [t, written])
                                .collect_vec(),
                        ))
                        .color(egui::Color32::BLUE),
                    );
                    plot_ui.line(
                        Line::new(PlotPoints::new(
                            history.iter().map(|(t, [_, read])| [t, read]).collect_vec(),
                        ))
                        .color(egui::Color32::RED),
                    );
                });
            });
        });
    }

    fn xyz_plot_ui(&mut self, ui: &mut egui::Ui, kind: ImuTabKind, max_width: f32) {
        ui.vertical(|ui| {
            let (history, display_name, unit) = match kind {
                ImuTabKind::Accel => (&mut self.state.accel_history, "Accelerometer", "(m/s^2)"),
                ImuTabKind::Gyro => (&mut self.state.gyro_history, "Gyroscope", "(rad/s)"),
                ImuTabKind::Mag => (&mut self.state.magnetometer_history, "Magnetometer", "(uT)"),
            };
            let Some(latest) = history.latest() else {
                ui.label(format!("No {display_name} data yet"));
                return;
            };
            ui.label(display_name);
            ui.add_sized([max_width, 150.0], |ui: &mut egui::Ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([max_width, 150.0], |ui: &mut egui::Ui| {
                        Plot::new(format!("{kind:?}"))
                            .allow_drag(false)
                            .allow_zoom(false)
                            .allow_scroll(false)
                            .show(ui, |plot_ui| {
                                for axis in Xyz::iter() {
                                    plot_ui.line(
                                        Line::new(PlotPoints::new(
                                            (*history)
                                                .iter()
                                                .map(|(t, v)| [t, v[axis as usize].into()])
                                                .collect_vec(),
                                        ))
                                        .color(match axis
                                        {
                                            Xyz::X => egui::Color32::RED,
                                            Xyz::Y => egui::Color32::GREEN,
                                            Xyz::Z => egui::Color32::BLUE,
                                        }),
                                    );
                                }
                            })
                            .response
                    });
                })
                .response
            });

            ui.label(format!(
                "{display_name}: ({:.2}, {:.2}, {:.2}) {unit}",
                latest[0], latest[1], latest[2]
            ));
        });
    }
}

impl<'a, 'b> TabViewer for StatsTabs<'a, 'b> {
    type Tab = StatTabKind;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            StatTabKind::Imu => {
                self.state.imu_tab_visible = true;
                self.imu_ui(ui);
            }
            StatTabKind::Xlink => {
                self.xlink_ui(ui);
            }
        };
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            StatTabKind::Imu => "IMU".into(),
            StatTabKind::Xlink => "Xlink".into(),
        }
    }
}
