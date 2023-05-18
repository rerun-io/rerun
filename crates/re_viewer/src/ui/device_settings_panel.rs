use crate::{depthai::depthai, misc::ViewerContext};

use strum::IntoEnumIterator; // Needed for enum::iter()

/// The "Selection View" side-bar.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub(crate) struct DeviceSettingsPanel {}

const CONFIG_UI_WIDTH: f32 = 224.0;

impl DeviceSettingsPanel {
    #[allow(clippy::unused_self)]
    pub fn show_panel(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        let mut available_devices = ctx.depthai_state.get_devices();
        let currently_selected_device = ctx.depthai_state.selected_device.clone();
        let mut combo_device: depthai::DeviceId = currently_selected_device.id;
        if !combo_device.is_empty() && available_devices.is_empty() {
            available_devices.push(combo_device.clone());
        }

        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                ..Default::default()
            })
            .show_inside(ui, |ui| {
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
                                            ctx.depthai_state.set_device(combo_device.clone());
                                        }
                                    }
                                },
                            );
                        })
                        .response
                    },
                );

                if ctx.depthai_state.applied_device_config.update_in_progress {
                    ui.add_sized([CONFIG_UI_WIDTH, 10.0], |ui: &mut egui::Ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Spinner::new())
                        })
                        .response
                    });
                    return;
                }
                Self::device_configuration_ui(ctx, ui);
            });
    }

    fn device_configuration_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        let mut device_config = ctx.depthai_state.modified_device_config.config.clone();
        let primary_700 = ctx.re_ui.design_tokens.primary_700;
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let mut style = ui.style_mut().clone();
                style.spacing.scroll_bar_inner_margin = 0.0;
                ui.set_style(style);
                egui::Frame {
                    fill: ctx.re_ui.design_tokens.gray_50,
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
                                        ui.set_width(CONFIG_UI_WIDTH);
                                        ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Resolution",
                                            format!("{}", device_config.color_camera.resolution),
                                            false,
                                            |ui| {
                                                for res in &ctx
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
                                        ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "FPS",
                                            &mut device_config.color_camera.fps,
                                            0..=120,
                                        );
                                        ctx.re_ui.labeled_checkbox(
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
                                        ui.set_width(CONFIG_UI_WIDTH);
                                        ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Resolution",
                                            format!("{}", device_config.left_camera.resolution),
                                            false,
                                            |ui| {
                                                for res in &ctx
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
                                        ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "FPS",
                                            &mut device_config.left_camera.fps,
                                            0..=120,
                                        );
                                        ctx.re_ui.labeled_checkbox(
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
                                        ui.set_width(CONFIG_UI_WIDTH);
                                        ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "Resolution",
                                            format!("{}", device_config.right_camera.resolution),
                                            false,
                                            |ui| {
                                                for res in &ctx
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
                                        ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "FPS",
                                            &mut device_config.right_camera.fps,
                                            0..=120,
                                        );
                                        ctx.re_ui.labeled_checkbox(
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
                                        ui.set_width(CONFIG_UI_WIDTH);
                                        ctx.re_ui.labeled_combo_box(
                                            ui,
                                            "AI Model",
                                            device_config.ai_model.display_name.clone(),
                                            false,
                                            |ui| {
                                                for nn in &ctx.depthai_state.neural_networks {
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
                                        ui.set_width(CONFIG_UI_WIDTH);
                                        ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "LR Check",
                                            &mut depth.lr_check,
                                        );
                                        ctx.re_ui.labeled_combo_box(
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
                                        ctx.re_ui.labeled_combo_box(
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
                                        ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "LR Threshold",
                                            &mut depth.lrc_threshold,
                                            0..=10,
                                        );
                                        ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "Extended Disparity",
                                            &mut depth.extended_disparity,
                                        );
                                        ctx.re_ui.labeled_checkbox(
                                            ui,
                                            "Subpixel Disparity",
                                            &mut depth.subpixel_disparity,
                                        );
                                        ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "Sigma",
                                            &mut depth.sigma,
                                            0..=65535,
                                        );
                                        ctx.re_ui.labeled_dragvalue(
                                            ui,
                                            "Confidence",
                                            &mut depth.confidence,
                                            0..=255,
                                        );
                                        ctx.re_ui.labeled_toggle_switch(
                                            ui,
                                            "Depth enabled",
                                            &mut device_config.depth_enabled,
                                        );
                                    });
                                },
                            );

                            device_config.depth = Some(depth);
                            ctx.depthai_state.modified_device_config.config = device_config.clone();
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    let only_runtime_configs_changed =
                                        depthai::State::only_runtime_configs_changed(
                                            &ctx.depthai_state.applied_device_config.config,
                                            &device_config,
                                        );
                                    let apply_enabled = !only_runtime_configs_changed
                                        && device_config
                                            != ctx.depthai_state.applied_device_config.config
                                        && !ctx.depthai_state.selected_device.id.is_empty();
                                    if !apply_enabled && only_runtime_configs_changed {
                                        ctx.depthai_state
                                            .set_device_config(&mut device_config, true);
                                    }
                                    if ctx.depthai_state.selected_device.id.is_empty() {
                                        ctx.depthai_state
                                            .set_device_config(&mut device_config, false);
                                    }

                                    ui.add_enabled_ui(apply_enabled, |ui| {
                                        ui.scope(|ui| {
                                            let mut style = ui.style_mut().clone();
                                            if apply_enabled {
                                                let color =
                                                    ctx.re_ui.design_tokens.primary_bg_color;
                                                let hover_color =
                                                    ctx.re_ui.design_tokens.primary_hover_bg_color;
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
                                                    [CONFIG_UI_WIDTH, re_ui::ReUi::box_height()],
                                                    egui::Button::new("Apply"),
                                                )
                                                .clicked()
                                            {
                                                ctx.depthai_state
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
}
