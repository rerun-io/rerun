use egui::{
    plot::{Line, Plot, PlotPoints},
    NumExt as _,
};
use re_data_store::{
    query_latest_single, ColorMapper, Colormap, EditableAutoValue, EntityPath, EntityProperties,
};

use itertools::Itertools;
use re_arrow_store::{LatestAtQuery, TimeInt, Timeline};
use re_log_types::{
    component_types::{ImuData, Tensor, TensorDataMeaning},
    Component, TimeType, Transform,
};

use crate::{
    depthai::depthai,
    ui::{view_spatial::SpatialNavigationMode, Blueprint},
    Item, UiVerbosity, ViewerContext,
};

use egui_dock::{DockArea, Tree};

use super::{data_ui::DataUi, space_view::ViewState};

use egui::emath::History;
use strum::EnumIter;
use strum::IntoEnumIterator; // Needed for enum::iter()

// ---

#[derive(Debug, Copy, Clone, EnumIter)]
enum XYZ {
    X,
    Y,
    Z,
}

#[derive(Debug, Copy, Clone)]
enum ImuTabKind {
    Accel,
    Gyro,
    Mag,
}

struct DepthaiTabs<'a, 'b> {
    ctx: &'a mut ViewerContext<'b>,
    accel_history: &'a mut History<[f32; 3]>,
    gyro_history: &'a mut History<[f32; 3]>,
    magnetometer_history: &'a mut History<[f32; 3]>,
    now: f64, // Time elapsed from spawning SelectionPanel
    unsubscribe_from_imu: bool,
    imu_visible: &'a mut bool,
}

impl<'a, 'b> DepthaiTabs<'a, 'b> {
    pub fn tree() -> Tree<String> {
        let config_tab = "Configuration".to_string();
        let imu_tab = "IMU".to_string();
        Tree::new(vec![config_tab, imu_tab])
    }

    fn device_configuration_ui(&mut self, ui: &mut egui::Ui) {
        // re_log::info!("pipeline_state: {:?}", pipeline_state);
        let mut device_config = self.ctx.depthai_state.modified_device_config.config.clone();
        let primary_700 = self.ctx.re_ui.design_tokens.primary_700;
        let gray_900 = self.ctx.re_ui.design_tokens.gray_900;
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let mut style = ui.style_mut().clone();
                style.spacing.scroll_bar_inner_margin = 0.0;
                ui.set_style(style);
                egui::Frame {
                    fill: self.ctx.re_ui.design_tokens.gray_50,
                    inner_margin: egui::Margin::symmetric(30.0, 21.0),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.collapsing(
                                egui::RichText::new("Color Camera").color(primary_700),
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(config_ui_width);
                                        self.ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Resolution",
                                            format!("{}", device_config.color_camera.resolution),
                                            false,
                                            |ui| {
                                                for res in &self
                                                    .ctx
                                                    .depthai_state
                                                    .selected_device
                                                    .supported_color_resolutions
                                                {
                                                    ui.selectable_value(
                                                        &mut device_config.color_camera.resolution,
                                                        *res,
                                                        format!("{res}"),
                                                    );
                                                }
                                            },
                                        );
                                        self.ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "FPS",
                                            &mut device_config.color_camera.fps,
                                            0..=120,
                                        );
                                        self.ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "Stream",
                                            &mut device_config.color_camera.stream_enabled,
                                        );
                                    });
                                },
                            );
                            ui.collapsing(
                                egui::RichText::new("Left Mono Camera").color(primary_700),
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(config_ui_width);
                                        self.ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Resolution",
                                            format!("{}", device_config.left_camera.resolution),
                                            false,
                                            |ui| {
                                                for res in &self
                                                    .ctx
                                                    .depthai_state
                                                    .selected_device
                                                    .supported_left_mono_resolutions
                                                {
                                                    ui.selectable_value(
                                                        &mut device_config.left_camera.resolution,
                                                        *res,
                                                        format!("{res}"),
                                                    );
                                                }
                                            },
                                        );
                                        self.ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "FPS",
                                            &mut device_config.left_camera.fps,
                                            0..=120,
                                        );
                                        self.ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "Stream",
                                            &mut device_config.left_camera.stream_enabled,
                                        );
                                    })
                                },
                            );

                            ui.collapsing(
                                egui::RichText::new("Right Mono Camera").color(primary_700),
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(config_ui_width);
                                        self.ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Resolution",
                                            format!("{}", device_config.right_camera.resolution),
                                            false,
                                            |ui| {
                                                for res in &self
                                                    .ctx
                                                    .depthai_state
                                                    .selected_device
                                                    .supported_right_mono_resolutions
                                                {
                                                    ui.selectable_value(
                                                        &mut device_config.right_camera.resolution,
                                                        *res,
                                                        format!("{res}"),
                                                    );
                                                }
                                            },
                                        );
                                        self.ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "FPS",
                                            &mut device_config.right_camera.fps,
                                            0..=120,
                                        );
                                        self.ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "Stream",
                                            &mut device_config.right_camera.stream_enabled,
                                        );
                                    })
                                },
                            );

                            // This is a hack, I wanted AI settings at the bottom, but some depth settings names
                            // are too long and it messes up the width of the ui layout somehow.
                            ui.collapsing(
                                egui::RichText::new("AI settings").color(primary_700),
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(config_ui_width);
                                        self.ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "AI Model",
                                            device_config.ai_model.display_name.clone(),
                                            false,
                                            |ui| {
                                                for nn in &self.ctx.depthai_state.neural_networks {
                                                    ui.selectable_value(
                                                        &mut device_config.ai_model,
                                                        nn.clone(),
                                                        &nn.display_name,
                                                    );
                                                }
                                            },
                                        );
                                    });
                                },
                            );

                            let mut depth = device_config.depth.unwrap_or_default();
                            if depth.align == depthai::BoardSocket::RGB && !depth.lr_check {
                                depth.align = depthai::BoardSocket::AUTO;
                            }

                            ui.collapsing(
                                egui::RichText::new("Depth settings").color(primary_700),
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(config_ui_width);
                                        self.ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "LR Check",
                                            &mut depth.lr_check,
                                        );
                                        self.ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Align to",
                                            format!("{:?}", depth.align),
                                            false,
                                            |ui| {
                                                for align in depthai::BoardSocket::iter() {
                                                    if align == depthai::BoardSocket::RGB
                                                        && !depth.lr_check
                                                    {
                                                        continue;
                                                    }
                                                    ui.selectable_value(
                                                        &mut depth.align,
                                                        align,
                                                        format!("{align:?}"),
                                                    );
                                                }
                                            },
                                        );
                                        self.ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Median Filter",
                                            format!("{:?}", depth.median),
                                            false,
                                            |ui| {
                                                for filter in depthai::DepthMedianFilter::iter() {
                                                    ui.selectable_value(
                                                        &mut depth.median,
                                                        filter,
                                                        format!("{filter:?}"),
                                                    );
                                                }
                                            },
                                        );
                                        self.ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "LR Threshold",
                                            &mut depth.lrc_threshold,
                                            0..=10,
                                        );
                                        self.ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "Extended Disparity",
                                            &mut depth.extended_disparity,
                                        );
                                        self.ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "Subpixel Disparity",
                                            &mut depth.subpixel_disparity,
                                        );
                                        self.ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "Sigma",
                                            &mut depth.sigma,
                                            0..=65535,
                                        );
                                        self.ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "Confidence",
                                            &mut depth.confidence,
                                            0..=255,
                                        );
                                        self.ctx.re_ui.labeled_toggle_switch(
                                            ui,
                                            "Depth enabled",
                                            &mut device_config.depth_enabled,
                                        );
                                    });
                                },
                            );

                            device_config.depth = Some(depth);
                            self.ctx.depthai_state.modified_device_config.config =
                                device_config.clone();
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    let only_runtime_configs_changed =
                                        depthai::State::only_runtime_configs_changed(
                                            &self.ctx.depthai_state.applied_device_config.config,
                                            &device_config,
                                        );
                                    let apply_enabled = !only_runtime_configs_changed
                                        && device_config
                                            != self.ctx.depthai_state.applied_device_config.config
                                        && !self.ctx.depthai_state.selected_device.id.is_empty();
                                    if !apply_enabled && only_runtime_configs_changed {
                                        self.ctx
                                            .depthai_state
                                            .set_device_config(&mut device_config, true);
                                    }
                                    if self.ctx.depthai_state.selected_device.id.is_empty() {
                                        self.ctx
                                            .depthai_state
                                            .set_device_config(&mut device_config, false);
                                    }

                                    ui.add_enabled_ui(apply_enabled, |ui| {
                                        ui.scope(|ui| {
                                            let mut style = ui.style_mut().clone();
                                            if apply_enabled {
                                                let color =
                                                    self.ctx.re_ui.design_tokens.primary_bg_color;
                                                let hover_color = self
                                                    .ctx
                                                    .re_ui
                                                    .design_tokens
                                                    .primary_hover_bg_color;
                                                style.visuals.widgets.hovered.bg_fill = hover_color;
                                                style.visuals.widgets.hovered.weak_bg_fill =
                                                    hover_color;
                                                style.visuals.widgets.inactive.bg_fill = color;
                                                style.visuals.widgets.inactive.weak_bg_fill = color;
                                                style.visuals.widgets.inactive.fg_stroke.color =
                                                    egui::Color32::WHITE;
                                                style.visuals.widgets.hovered.fg_stroke.color =
                                                    egui::Color32::WHITE;
                                            }
                                            style.spacing.button_padding =
                                                egui::Vec2::new(24.0, 4.0);
                                            ui.set_style(style);
                                            if ui
                                                .add_sized(
                                                    [config_ui_width, re_ui::ReUi::box_height()],
                                                    egui::Button::new("Apply"),
                                                )
                                                .clicked()
                                            {
                                                self.ctx
                                                    .depthai_state
                                                    .set_device_config(&mut device_config, false);
                                            }
                                        });
                                    });
                                });
                            });
                        });
                        ui.add_space(ui.available_width());
                    });
                });
            });
    }

    fn imu_ui(&mut self, ui: &mut egui::Ui) {
        let imu_entity_path = &ImuData::entity_path();

        if let Ok(latest) = re_query::query_entity_with_primary::<ImuData>(
            &self.ctx.log_db.entity_db.data_store,
            &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
            imu_entity_path,
            &[ImuData::name()],
        ) {
            latest.visit1(|_inst, imu_data| {
                self.accel_history.add(
                    self.now,
                    [imu_data.accel.x, imu_data.accel.y, imu_data.accel.z],
                );
                self.gyro_history.add(
                    self.now,
                    [imu_data.gyro.x, imu_data.gyro.y, imu_data.gyro.z],
                );
                if let Some(mag) = imu_data.mag {
                    self.magnetometer_history
                        .add(self.now, [mag.x, mag.y, mag.z]);
                }
            });
        }

        let tab_kinds = [ImuTabKind::Accel, ImuTabKind::Gyro, ImuTabKind::Mag];
        egui::ScrollArea::both().show(ui, |ui| {
            let max_width = ui.available_width();
            for kind in tab_kinds.iter() {
                self.xyz_plot_ui(ui, *kind, max_width);
            }
        });
    }

    fn xyz_plot_ui(&mut self, ui: &mut egui::Ui, kind: ImuTabKind, max_width: f32) {
        ui.vertical(|ui| {
            let (history, display_name, unit) = match kind {
                ImuTabKind::Accel => (&mut self.accel_history, "Accelerometer", "(m/s^2)"),
                ImuTabKind::Gyro => (&mut self.gyro_history, "Gyroscope", "(rad/s)"),
                ImuTabKind::Mag => (&mut self.magnetometer_history, "Magnetometer", "(uT)"),
            };
            let Some(latest) = history.latest() else {
        ui.label(format!("No {display_name} data yet"));
        return;
    };
            ui.label(display_name);
            ui.add_sized([max_width, 150.0], |ui: &mut egui::Ui| {
                ui.horizontal(|ui| {
                    for axis in XYZ::iter() {
                        ui.add_sized([max_width / 3.0, 150.0], |ui: &mut egui::Ui| {
                            Plot::new(format!("{kind:?} ({axis:?})"))
                                .allow_drag(false)
                                .allow_zoom(false)
                                .allow_scroll(false)
                                .show(ui, |plot_ui| {
                                    plot_ui.line(Line::new(PlotPoints::new(
                                        (*history)
                                            .iter()
                                            .map(|(t, v)| [t, v[axis as usize].into()])
                                            .collect_vec(),
                                    )));
                                })
                                .response
                        });
                    }
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

impl<'a, 'b> egui_dock::TabViewer for DepthaiTabs<'a, 'b> {
    type Tab = String;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "Configuration" => {
                // Unsubscribe from IMU data if subscribed
                if self.unsubscribe_from_imu
                    && self
                        .ctx
                        .depthai_state
                        .subscriptions
                        .contains(&depthai::ChannelId::ImuData)
                {
                    let mut subs = self
                        .ctx
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
                    self.ctx.depthai_state.set_subscriptions(&subs);
                    self.accel_history.clear();
                    self.gyro_history.clear();
                    self.magnetometer_history.clear();
                }
                self.device_configuration_ui(ui);
            }
            "IMU" => {
                *self.imu_visible = true;
                // Subscribe to IMU data if not already subscribed
                if !self
                    .ctx
                    .depthai_state
                    .subscriptions
                    .contains(&depthai::ChannelId::ImuData)
                {
                    let mut subs = self.ctx.depthai_state.subscriptions.clone();
                    subs.push(depthai::ChannelId::ImuData);
                    self.ctx.depthai_state.set_subscriptions(&subs);
                }
                self.imu_ui(ui);
            }
            _ => {}
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }
}

/// The "Selection View" side-bar.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SelectionPanel {
    #[serde(skip)]
    depthai_tabs: Tree<String>,
    #[serde(skip)]
    accel_history: History<[f32; 3]>,
    #[serde(skip)]
    gyro_history: History<[f32; 3]>,
    #[serde(skip)]
    magnetometer_history: History<[f32; 3]>,
    #[serde(skip)]
    start_time: instant::Instant,
    #[serde(skip)]
    current_device_config_panel_min_height: f32, // A bit hacky, used to keep the top panel from becoming really small after showing spinner
    #[serde(skip)]
    device_config_panel_height: f32, // Used to reset height to previous height after config load
    #[serde(skip)]
    imu_tab_visible: bool, // Used to subscribe to IMU data when the imu tab is shown, or rather unsubscribe when it's not (enables the user to view both the imu and the configuration at the same time)
    #[serde(skip)]
    apply_cfg_button_enabled: bool, // Used to disable the apply button when the config has changed, keeps the state between frames
}

impl Default for SelectionPanel {
    fn default() -> Self {
        Self {
            depthai_tabs: DepthaiTabs::tree(),
            accel_history: History::new(0..1000, 5.0),
            gyro_history: History::new(0..1000, 5.0),
            magnetometer_history: History::new(0..1000, 5.0),
            start_time: instant::Instant::now(),
            current_device_config_panel_min_height: 0.0,
            device_config_panel_height: 500.0,
            imu_tab_visible: false,
            apply_cfg_button_enabled: false,
        }
    }
}

const config_ui_width: f32 = 224.0;

impl SelectionPanel {
    #[allow(clippy::unused_self)]
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        blueprint: &mut Blueprint,
    ) {
        let screen_width = ui.ctx().screen_rect().width();

        let panel = egui::SidePanel::right("selection_view")
            .min_width(120.0)
            .default_width((0.45 * screen_width).min(250.0).round())
            .max_width((0.65 * screen_width).round())
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.style().visuals.panel_fill,
                ..Default::default()
            });

        panel.show_animated_inside(
            ui,
            blueprint.selection_panel_expanded,
            |ui: &mut egui::Ui| {
                let response_rect = egui::TopBottomPanel::top("Device configuration")
                    .resizable(true)
                    .min_height(self.current_device_config_panel_min_height)
                    .show_separator_line(true)
                    .frame(egui::Frame {
                        inner_margin: egui::Margin::symmetric(
                            re_ui::ReUi::view_padding(),
                            re_ui::ReUi::view_padding(),
                        ),
                        ..Default::default()
                    })
                    .show_inside(ui, |ui| {
                        let mut available_devices = ctx.depthai_state.get_devices();
                        let currently_selected_device = ctx.depthai_state.selected_device.clone();
                        let mut combo_device: depthai::DeviceId = currently_selected_device.id;
                        if !combo_device.is_empty() && available_devices.is_empty() {
                            available_devices.push(combo_device.clone());
                        }
                        ui.add_sized(
                            [ui.available_width(), re_ui::ReUi::box_height()],
                            |ui: &mut egui::Ui| {
                                ui.horizontal(|ui| {
                                    ctx.re_ui.labeled_combo_box(
                                        ui,
                                        "Device",
                                        if !combo_device.is_empty() {
                                            combo_device.clone()
                                        } else {
                                            "No device selected".to_owned()
                                        },
                                        true,
                                        |ui: &mut egui::Ui| {
                                            if ui
                                                .selectable_value(
                                                    &mut combo_device,
                                                    String::new(),
                                                    "No device",
                                                )
                                                .changed()
                                            {
                                                ctx.depthai_state.set_device(combo_device.clone());
                                            }
                                            for device in available_devices {
                                                if ui
                                                    .selectable_value(
                                                        &mut combo_device,
                                                        device.clone(),
                                                        device,
                                                    )
                                                    .changed()
                                                {
                                                    ctx.depthai_state
                                                        .set_device(combo_device.clone());
                                                }
                                            }
                                        },
                                    );
                                })
                                .response
                            },
                        );

                        if ctx.depthai_state.applied_device_config.update_in_progress {
                            ui.add_sized([config_ui_width, 10.0], |ui: &mut egui::Ui| {
                                ui.with_layout(
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| ui.add(egui::Spinner::new()),
                                )
                                .response
                            });
                            // The following lines are a hack to force the top panel to resize to a usable size
                            // after updating the device config, when updating set min height to 10 then detect if
                            // it's 10 the config has been updated, set the panel to be of size 200.0, then in the next frame
                            // set min height to 20.0 so user can still resize the panel to be very small
                            self.current_device_config_panel_min_height = 10.0;
                            return;
                        } else if self.current_device_config_panel_min_height == 10.0 {
                            self.current_device_config_panel_min_height =
                                self.device_config_panel_height;
                        } else {
                            self.current_device_config_panel_min_height = 20.0;
                        }
                        let mut imu_tab_visible = false;
                        let unsubscribe_from_imu = !self.imu_tab_visible;
                        DockArea::new(&mut self.depthai_tabs)
                            .id(egui::Id::new("depthai_tabs"))
                            .style(re_ui::egui_dock_style(ui.style()))
                            .show_inside(
                                ui,
                                &mut DepthaiTabs {
                                    ctx,
                                    accel_history: &mut self.accel_history,
                                    gyro_history: &mut self.gyro_history,
                                    magnetometer_history: &mut self.magnetometer_history,
                                    now: self.start_time.elapsed().as_nanos() as f64 / 1e9,
                                    unsubscribe_from_imu,
                                    imu_visible: &mut imu_tab_visible,
                                },
                            );
                        self.imu_tab_visible = imu_tab_visible;
                    })
                    .response
                    .rect;
                // When panel isn't small keep remembering the height of the panel
                if self.current_device_config_panel_min_height != 10.0 {
                    self.device_config_panel_height = (response_rect.max - response_rect.min).y;
                }

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    egui::TopBottomPanel::top("selection_panel_title_bar")
                        .exact_height(re_ui::ReUi::title_bar_height())
                        .frame(egui::Frame {
                            inner_margin: egui::Margin::symmetric(re_ui::ReUi::view_padding(), 0.0),
                            ..Default::default()
                        })
                        .show_inside(ui, |ui| {
                            if let Some(selection) = ctx
                                .rec_cfg
                                .selection_state
                                .selection_ui(ctx.re_ui, ui, blueprint)
                            {
                                ctx.set_multi_selection(selection.iter().cloned());
                            }
                        });

                    egui::ScrollArea::both()
                        .auto_shrink([true; 2])
                        .show(ui, |ui| {
                            egui::Frame {
                                inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                                ..Default::default()
                            }
                            .show(ui, |ui| {
                                self.contents(ui, ctx, blueprint);
                            });
                        });
                });
            },
        );
    }

    #[allow(clippy::unused_self)]
    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
    ) {
        crate::profile_function!();

        let query = ctx.current_query();

        if ctx.selection().is_empty() {
            return;
        }

        let num_selections = ctx.selection().len();
        let selection = ctx.selection().to_vec();
        for (i, item) in selection.iter().enumerate() {
            ui.push_id(i, |ui| {
                what_is_selected_ui(ui, ctx, blueprint, item);

                if has_data_section(item) {
                    ctx.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                        item.data_ui(ctx, ui, UiVerbosity::All, &query);
                    });
                }

                ctx.re_ui
                    .large_collapsing_header(ui, "Blueprint", true, |ui| {
                        blueprint_ui(ui, ctx, blueprint, item);
                    });

                if i + 1 < num_selections {
                    // Add space some space between selections
                    ui.add(egui::Separator::default().spacing(24.0).grow(20.0));
                }
            });
        }
    }
}

fn has_data_section(item: &Item) -> bool {
    match item {
        Item::ComponentPath(_) | Item::InstancePath(_, _) => true,
        // Skip data ui since we don't know yet what to show for these.
        Item::SpaceView(_) | Item::DataBlueprintGroup(_, _) => false,
    }
}

/// What is selected? Not the contents, just the short id of it.
pub fn what_is_selected_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    item: &Item,
) {
    match item {
        Item::ComponentPath(re_log_types::ComponentPath {
            entity_path,
            component_name,
        }) => {
            egui::Grid::new("component_path")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Entity:");
                    ctx.entity_path_button(ui, None, entity_path);
                    ui.end_row();

                    ui.label("Component:");
                    ui.label(component_name.short_name())
                        .on_hover_text(component_name.full_name());
                    ui.end_row();
                });
        }
        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                ui.horizontal(|ui| {
                    ui.label("Space view:");
                    ui.text_edit_singleline(&mut space_view.display_name);
                });
            }
        }
        Item::InstancePath(space_view_id, instance_path) => {
            egui::Grid::new("space_view_id_entity_path").show(ui, |ui| {
                if instance_path.instance_key.is_splat() {
                    ui.label("Entity:");
                } else {
                    ui.label("Entity instance:");
                }
                ctx.instance_path_button(ui, *space_view_id, instance_path);
                ui.end_row();

                if let Some(space_view_id) = space_view_id {
                    if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                        ui.label("in Space View:");
                        ctx.space_view_button(ui, space_view);
                        ui.end_row();
                    }
                }
            });
        }
        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    egui::Grid::new("data_blueprint_group")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Data Group:");
                            ctx.data_blueprint_group_button_to(
                                ui,
                                group.display_name.clone(),
                                space_view.id,
                                *data_blueprint_group_handle,
                            );
                            ui.end_row();

                            ui.label("in Space View:");
                            ctx.space_view_button_to(
                                ui,
                                space_view.display_name.clone(),
                                space_view.id,
                                space_view.category,
                            );
                            ui.end_row();
                        });
                }
            }
        }
    }
}

impl DataUi for Item {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Item::SpaceView(_) | Item::DataBlueprintGroup(_, _) => {
                // Shouldn't be reachable since SelectionPanel::contents doesn't show data ui for these.
                // If you add something in here make sure to adjust SelectionPanel::contents accordingly.
                debug_assert!(!has_data_section(self));
            }
            Item::ComponentPath(component_path) => {
                component_path.data_ui(ctx, ui, verbosity, query);
            }
            Item::InstancePath(_, instance_path) => {
                instance_path.data_ui(ctx, ui, verbosity, query);
            }
        }
    }
}

/// What is the blueprint stuff for this item?
fn blueprint_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    item: &Item,
) {
    match item {
        Item::ComponentPath(component_path) => {
            list_existing_data_blueprints(ui, ctx, component_path.entity_path(), blueprint);
        }

        Item::SpaceView(space_view_id) => {
            ui.horizontal(|ui| {
                if ui
                    .button("Add/remove entities")
                    .on_hover_text("Manually add or remove entities from the Space View.")
                    .clicked()
                {
                    blueprint
                        .viewport
                        .show_add_remove_entities_window(*space_view_id);
                }

                if ui
                    .button("Clone view")
                    .on_hover_text("Create an exact duplicate of this Space View including all blueprint settings")
                    .clicked()
                {
                    if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                        let mut new_space_view = space_view.clone();
                        new_space_view.id = super::SpaceViewId::random();
                        blueprint.viewport.add_space_view(new_space_view);
                        blueprint.viewport.mark_user_interaction();
                    }
                }
            });

            ui.add_space(ui.spacing().item_spacing.y);

            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                space_view.selection_ui(ctx, ui);
            }
        }

        Item::InstancePath(space_view_id, instance_path) => {
            if let Some(space_view) = space_view_id
                .and_then(|space_view_id| blueprint.viewport.space_view_mut(&space_view_id))
            {
                if instance_path.instance_key.is_specific() {
                    ui.horizontal(|ui| {
                        ui.label("Part of");
                        ctx.entity_path_button(ui, *space_view_id, &instance_path.entity_path);
                    });
                    // TODO(emilk): show the values of this specific instance (e.g. point in the point cloud)!
                } else {
                    // splat - the whole entity
                    let data_blueprint = space_view.data_blueprint.data_blueprints_individual();
                    let mut props = data_blueprint.get(&instance_path.entity_path);
                    entity_props_ui(
                        ctx,
                        ui,
                        Some(&instance_path.entity_path),
                        &mut props,
                        &space_view.view_state,
                    );
                    data_blueprint.set(instance_path.entity_path.clone(), props);
                }
            } else {
                list_existing_data_blueprints(ui, ctx, &instance_path.entity_path, blueprint);
            }
        }

        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    entity_props_ui(
                        ctx,
                        ui,
                        None,
                        &mut group.properties_individual,
                        &space_view.view_state,
                    );
                } else {
                    ctx.selection_state_mut().clear_current();
                }
            }
        }
    }
}

fn list_existing_data_blueprints(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    entity_path: &EntityPath,
    blueprint: &Blueprint,
) {
    let space_views_with_path = blueprint
        .viewport
        .space_views_containing_entity_path(entity_path);

    if space_views_with_path.is_empty() {
        ui.weak("(Not shown in any Space View)");
        // TODO(andreas): Offer options for adding?
    } else {
        ui.label("Is shown in:");

        ui.indent("list of data blueprints indent", |ui| {
            for space_view_id in &space_views_with_path {
                if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                    ctx.entity_path_button_to(
                        ui,
                        Some(*space_view_id),
                        entity_path,
                        &space_view.display_name,
                    );
                }
            }
        });
    }
}

fn entity_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: Option<&EntityPath>,
    entity_props: &mut EntityProperties,
    view_state: &ViewState,
) {
    ui.checkbox(&mut entity_props.visible, "Visible");
    ui.checkbox(&mut entity_props.interactive, "Interactive")
        .on_hover_text("If disabled, the entity will not react to any mouse interaction");

    egui::Grid::new("entity_properties")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Visible history");
            let visible_history = &mut entity_props.visible_history;
            match ctx.rec_cfg.time_ctrl.timeline().typ() {
                TimeType::Time => {
                    let mut time_sec = visible_history.nanos as f32 / 1e9;
                    let speed = (time_sec * 0.05).at_least(0.01);
                    ui.add(
                        egui::DragValue::new(&mut time_sec)
                            .clamp_range(0.0..=f32::INFINITY)
                            .speed(speed)
                            .suffix("s"),
                    )
                    .on_hover_text("Include this much history of the Entity in the Space View.");
                    visible_history.nanos = (time_sec * 1e9).round() as _;
                }
                TimeType::Sequence => {
                    let speed = (visible_history.sequences as f32 * 0.05).at_least(1.0);
                    ui.add(
                        egui::DragValue::new(&mut visible_history.sequences)
                            .clamp_range(0.0..=f32::INFINITY)
                            .speed(speed),
                    )
                    .on_hover_text("Include this much history of the Entity in the Space View.");
                }
            }
            ui.end_row();

            if *view_state.state_spatial.nav_mode.get() == SpatialNavigationMode::ThreeD {
                if let Some(entity_path) = entity_path {
                    pinhole_props_ui(ctx, ui, entity_path, entity_props);
                    depth_props_ui(ctx, ui, entity_path, entity_props);
                }
            }
        });
}

fn colormap_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    // Color mapping picker
    {
        let current = *entity_props.color_mapper.get();
        ui.label("Color map");
        egui::ComboBox::from_id_source("depth_color_mapper")
            .selected_text(current.to_string())
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.set_min_width(64.0);

                let mut add_label = |proposed| {
                    if ui
                        .selectable_label(current == proposed, proposed.to_string())
                        .clicked()
                    {
                        entity_props.color_mapper = EditableAutoValue::Auto(proposed);
                    }
                };

                add_label(ColorMapper::Colormap(Colormap::Grayscale));
                add_label(ColorMapper::Colormap(Colormap::Turbo));
                add_label(ColorMapper::Colormap(Colormap::Viridis));
                add_label(ColorMapper::Colormap(Colormap::Plasma));
                add_label(ColorMapper::Colormap(Colormap::Magma));
                add_label(ColorMapper::Colormap(Colormap::Inferno));
                add_label(ColorMapper::AlbedoTexture);
            });
        ui.end_row();
    }

    if *entity_props.color_mapper.get() != ColorMapper::AlbedoTexture {
        return;
    }

    // Albedo texture picker
    if let Some(tree) = entity_path
        .parent()
        .and_then(|path| ctx.log_db.entity_db.tree.subtree(&path))
    {
        let query = ctx.current_query();
        let current = entity_props.albedo_texture.clone();

        ui.label("Albedo texture");

        let mut combo = egui::ComboBox::from_id_source("depth_color_texture");
        if let Some(current) = current.as_ref() {
            combo = combo.selected_text(current.to_string());
        } else {
            // Select the first image-shaped tensor we find
            // tree.visit_children_recursively(&mut |ent_path| {
            //     if entity_props.albedo_texture.is_some() {
            //         return;
            //     }
            //     let Some(tensor) =
            //         query_latest_single::<Tensor>(&ctx.log_db.entity_db, ent_path, &query) else {
            //             return;
            //         };
            //     if tensor.is_shaped_like_an_image() {
            //         entity_props.albedo_texture = Some(ent_path.clone());
            //     }
            // });
        }

        combo.show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(64.0);

            tree.visit_children_recursively(&mut |ent_path| {
                let Some(tensor) = query_latest_single::<Tensor>(
                    &ctx.log_db.entity_db,
                    ent_path,
                    &query,
                ) else {
                    return;
                };

                if tensor.is_shaped_like_an_image()
                    && ui
                        .selectable_label(current.as_ref() == Some(ent_path), ent_path.to_string())
                        .clicked()
                {
                    entity_props.albedo_texture = Some(ent_path.clone());
                }
            });
        });
    }
}

fn pinhole_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    let query = ctx.current_query();
    if let Some(re_log_types::Transform::Pinhole(_)) =
        query_latest_single::<Transform>(&ctx.log_db.entity_db, entity_path, &query)
    {
        ui.label("Image plane distance");
        let mut distance = *entity_props.pinhole_image_plane_distance.get();
        let speed = (distance * 0.05).at_least(0.01);
        if ui
            .add(
                egui::DragValue::new(&mut distance)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text("Controls how far away the image plane is.")
            .changed()
        {
            entity_props.pinhole_image_plane_distance = EditableAutoValue::UserEdited(distance);
        }
        ui.end_row();
    }
}

fn depth_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) -> Option<()> {
    crate::profile_function!();

    let query = ctx.current_query();
    let tensor = query_latest_single::<Tensor>(&ctx.log_db.entity_db, entity_path, &query)?;
    if tensor.meaning != TensorDataMeaning::Depth {
        return Some(());
    }
    let pinhole_ent_path =
        crate::misc::queries::closest_pinhole_transform(ctx, entity_path, &query)?;

    let mut backproject_depth = *entity_props.backproject_depth.get();

    if ui
        .checkbox(&mut backproject_depth, "Backproject Depth")
        .on_hover_text(
            "If enabled, the depth texture will be backprojected into a point cloud rather \
                than simply displayed as an image.",
        )
        .changed()
    {
        entity_props.backproject_depth = EditableAutoValue::UserEdited(backproject_depth);
    }
    ui.end_row();

    if backproject_depth {
        ui.label("Pinhole");
        ctx.entity_path_button(ui, None, &pinhole_ent_path)
            .on_hover_text(
                "The entity path of the pinhole transform being used to do the backprojection.",
            );
        ui.end_row();

        depth_from_world_scale_ui(ui, &mut entity_props.depth_from_world_scale);

        backproject_radius_scale_ui(ui, &mut entity_props.backproject_radius_scale);

        ui.label("Backproject radius scale");
        let mut radius_scale = *entity_props.backproject_radius_scale.get();
        let speed = (radius_scale * 0.001).at_least(0.001);
        if ui
            .add(
                egui::DragValue::new(&mut radius_scale)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text("Scales the radii of the points in the backprojected point cloud")
            .changed()
        {
            entity_props.backproject_radius_scale = EditableAutoValue::UserEdited(radius_scale);
        }
        ui.end_row();

        // TODO(cmc): This should apply to the depth map entity as a whole, but for that we
        // need to get the current hardcoded colormapping out of the image cache first.
        colormap_props_ui(ctx, ui, entity_path, entity_props);
    }

    Some(())
}

fn depth_from_world_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject meter");
    let mut value = *property.get();
    let speed = (value * 0.05).at_least(0.01);

    let response = ui
    .add(
        egui::DragValue::new(&mut value)
            .clamp_range(0.0..=1.0e8)
            .speed(speed),
    )
    .on_hover_text("How many steps in the depth image correspond to one world-space unit. For instance, 1000 means millimeters.\n\
                    Double-click to reset.");
    if response.double_clicked() {
        // reset to auto - the exact value will be restored somewhere else
        *property = EditableAutoValue::Auto(value);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}

fn backproject_radius_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject radius scale");
    let mut value = *property.get();
    let speed = (value * 0.01).at_least(0.001);
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .clamp_range(0.0..=1.0e8)
                .speed(speed),
        )
        .on_hover_text(
            "Scales the radii of the points in the backprojected point cloud.\n\
            This is a factor of the projected pixel diameter. \
            This means a scale of 0.5 will leave adjacent pixels at the same depth value just touching.\n\
            Double-click to reset.",
        );
    if response.double_clicked() {
        *property = EditableAutoValue::Auto(2.0);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}
