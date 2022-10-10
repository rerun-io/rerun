use egui::Vec2;

use re_data_store::InstanceId;
pub use re_log_types::*;

use crate::misc::ViewerContext;

use super::Preview;

pub(crate) fn view_object(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    obj_path: &ObjPath,
    preview: Preview,
) -> Option<()> {
    view_instance(
        ctx,
        ui,
        &InstanceId {
            obj_path: obj_path.clone(),
            instance_index: None,
        },
        preview,
    )
}

pub(crate) fn view_instance(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_id: &InstanceId,
    preview: Preview,
) -> Option<()> {
    let store = ctx
        .log_db
        .obj_db
        .store
        .get(ctx.rec_cfg.time_ctrl.timeline())?;
    let time_query = ctx.rec_cfg.time_ctrl.time_query()?;
    let obj_store = store.get(&instance_id.obj_path)?;

    egui::Grid::new("object_instance")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            for (field_name, field_store) in obj_store.iter() {
                ctx.data_path_button_to(
                    ui,
                    field_name.to_string(),
                    &DataPath::new(instance_id.obj_path.clone(), *field_name),
                );

                match field_store
                    .query_field_to_datavec(&time_query, instance_id.instance_index.as_ref())
                {
                    Ok((time_msgid_index, data_vec)) => {
                        if data_vec.len() == 1 {
                            let data = data_vec.last().unwrap();
                            let (_, msg_id, _) = &time_msgid_index[0];
                            crate::data_ui::ui_data(ctx, ui, msg_id, &data, preview);
                        } else {
                            ui_data_vec(ui, &data_vec);
                        }
                    }
                    Err(err) => {
                        re_log::warn_once!("Bad data for {instance_id}: {err}");
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            format!("Data error: {:?}", err),
                        );
                    }
                }

                ui.end_row();
            }
        });

    Some(())
}

pub(crate) fn view_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    data_path: &DataPath,
) -> Option<()> {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let time_query = ctx.rec_cfg.time_ctrl.time_query()?;

    match ctx
        .log_db
        .obj_db
        .store
        .query_data_path(timeline, &time_query, data_path)?
    {
        Ok((time_msgid_index, data_vec)) => {
            if data_vec.len() == 1 {
                let data = data_vec.last().unwrap();
                let (_, msg_id, _) = &time_msgid_index[0];
                show_detailed_data(ctx, ui, msg_id, &data);
            } else {
                ui_data_vec(ui, &data_vec);
            }
        }
        Err(err) => {
            re_log::warn_once!("Bad data for {data_path}: {err}");
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Data error: {:?}", err),
            );
        }
    }

    Some(())
}

pub(crate) fn show_detailed_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg_id: &MsgId,
    data: &Data,
) {
    if let Data::Tensor(tensor) = data {
        crate::image_ui::show_tensor(ctx, ui, msg_id, tensor);
    } else {
        crate::data_ui::ui_data(ctx, ui, msg_id, data, Preview::Medium);
    }
}

pub(crate) fn show_detailed_data_msg(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &DataMsg,
) {
    let DataMsg {
        msg_id,
        time_point,
        data_path,
        data,
    } = msg;

    let is_image = matches!(msg.data, LoggedData::Single(Data::Tensor(_)));

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("data_path:");
            ctx.data_path_button(ui, data_path);
            ui.end_row();
            ui.monospace("object type path:");
            ctx.type_path_button(ui, data_path.obj_path.obj_type_path());
            ui.end_row();

            ui.monospace("time_point:");
            crate::data_ui::ui_time_point(ctx, ui, time_point);
            ui.end_row();

            if !is_image {
                ui.monospace("data:");
                crate::data_ui::ui_logged_data(ctx, ui, msg_id, data, Preview::Medium);
                ui.end_row();
            }
        });

    if let LoggedData::Single(Data::Tensor(tensor)) = &msg.data {
        crate::image_ui::show_tensor(ctx, ui, msg_id, tensor);
    }
}

// ----------------------------------------------------------------------------

pub(crate) fn show_log_msg(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &LogMsg,
    preview: Preview,
) {
    match msg {
        LogMsg::BeginRecordingMsg(msg) => show_begin_recording_msg(ui, msg),
        LogMsg::TypeMsg(msg) => show_type_msg(ctx, ui, msg),
        LogMsg::DataMsg(msg) => {
            show_data_msg(ctx, ui, msg, preview);
        }
    }
}

pub(crate) fn show_begin_recording_msg(ui: &mut egui::Ui, msg: &BeginRecordingMsg) {
    ui.code("BeginRecordingMsg");
    let BeginRecordingMsg { msg_id: _, info } = msg;
    let RecordingInfo {
        recording_id,
        started,
        recording_source,
    } = info;

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("recording_id:");
            ui.label(format!("{recording_id:?}"));
            ui.end_row();

            ui.monospace("started:");
            ui.label(started.format());
            ui.end_row();

            ui.monospace("recording_source:");
            ui.label(format!("{recording_source}"));
            ui.end_row();
        });
}

pub(crate) fn show_type_msg(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, msg: &TypeMsg) {
    ui.horizontal(|ui| {
        ctx.type_path_button(ui, &msg.type_path);
        ui.label(" = ");
        ui.code(format!("{:?}", msg.obj_type));
    });
}

pub(crate) fn show_data_msg(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &DataMsg,
    preview: Preview,
) {
    let DataMsg {
        msg_id,
        time_point,
        data_path,
        data,
    } = msg;

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("data_path:");
            ui.label(format!("{data_path}"));
            ui.end_row();

            ui.monospace("time_point:");
            ui_time_point(ctx, ui, time_point);
            ui.end_row();

            ui.monospace("data:");
            ui_logged_data(ctx, ui, msg_id, data, preview);
            ui.end_row();
        });
}

pub(crate) fn ui_time_point(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    time_point: &TimePoint,
) {
    ui.vertical(|ui| {
        egui::Grid::new("time_point").num_columns(2).show(ui, |ui| {
            for (timeline, value) in &time_point.0 {
                ui.label(format!("{}:", timeline.name()));
                ctx.time_button(ui, timeline, *value);
                ui.end_row();
            }
        });
    });
}

pub(crate) fn ui_logged_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg_id: &MsgId,
    data: &LoggedData,
    preview: Preview,
) -> egui::Response {
    match data {
        LoggedData::Batch { data, .. } => ui.label(format!("batch: {:?}", data)),
        LoggedData::Single(data) => ui_data(ctx, ui, msg_id, data, preview),
        LoggedData::BatchSplat(data) => {
            ui.horizontal(|ui| {
                ui.label("Batch Splat:");
                ui_data(ctx, ui, msg_id, data, preview)
            })
            .response
        }
    }
}

pub(crate) fn ui_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg_id: &MsgId,
    data: &Data,
    preview: Preview,
) -> egui::Response {
    match data {
        Data::Bool(value) => ui.label(value.to_string()),
        Data::I32(value) => ui.label(value.to_string()),
        Data::F32(value) => ui.label(value.to_string()),
        Data::Color([r, g, b, a]) => {
            let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
            let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
            ui.painter().rect_stroke(
                response.rect,
                1.0,
                ui.visuals().widgets.noninteractive.fg_stroke,
            );
            response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
        }
        Data::String(string) => ui.label(format!("{string:?}")),

        Data::Vec2([x, y]) => ui.label(format!("[{x:.1}, {y:.1}]")),
        Data::BBox2D(bbox) => ui.label(format!(
            "BBox2D(min: [{:.1} {:.1}], max: [{:.1} {:.1}])",
            bbox.min[0], bbox.min[1], bbox.max[0], bbox.max[1]
        )),

        Data::Vec3([x, y, z]) => ui.label(format!("[{x:.3}, {y:.3}, {z:.3}]")),
        Data::Box3(_) => ui.label("3D box"),
        Data::Mesh3D(_) => ui.label("3D mesh"),
        Data::Camera(cam) => match preview {
            Preview::Small | Preview::Specific(_) => ui.label("Camera"),
            Preview::Medium => ui_camera(ui, cam),
        },
        Data::Transform(transform) => match preview {
            Preview::Small | Preview::Specific(_) => ui.label("Transform"),
            Preview::Medium => ui_transform(ui, transform),
        },

        Data::Tensor(tensor) => {
            let egui_image = ctx.cache.image.get(msg_id, tensor);
            ui.horizontal_centered(|ui| {
                let max_width = match preview {
                    Preview::Small => 32.0,
                    Preview::Medium => 128.0,
                    Preview::Specific(height) => height,
                };

                egui_image
                    .show_max_size(ui, Vec2::new(4.0 * max_width, max_width))
                    .on_hover_ui(|ui| {
                        egui_image.show(ui);
                    });

                ui.vertical(|ui| {
                    ui.set_min_width(100.0);
                    ui.label(format!("dtype: {:?}", tensor.dtype));

                    if tensor.shape.len() == 2 {
                        ui.label(format!("shape: {:?} (height, width)", tensor.shape));
                    } else if tensor.shape.len() == 3 {
                        ui.label(format!("shape: {:?} (height, width, depth)", tensor.shape));
                    } else {
                        ui.label(format!("shape: {:?}", tensor.shape));
                    }
                });
            })
            .response
        }

        Data::Space(space) => {
            // ui.label(space.to_string())
            ctx.space_button(ui, space)
        }

        Data::DataVec(data_vec) => ui_data_vec(ui, data_vec),
    }
}

pub(crate) fn ui_data_vec(ui: &mut egui::Ui, data_vec: &DataVec) -> egui::Response {
    ui.label(format!(
        "{} x {:?}",
        data_vec.len(),
        data_vec.element_data_type(),
    ))
}

fn ui_camera(ui: &mut egui::Ui, cam: &Camera) -> egui::Response {
    let Camera {
        extrinsics,
        intrinsics,
        target_space,
    } = cam;

    let Extrinsics {
        rotation,
        position,
        camera_space_convention,
    } = extrinsics;

    ui.vertical(|ui| {
        ui.label("Camera");
        ui.indent("camera", |ui| {
            egui::Grid::new("camera")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("rotation");
                    ui.monospace(format!("{rotation:?}"));
                    ui.end_row();

                    ui.label("position");
                    ui.monospace(format!("{position:?}"));
                    ui.end_row();

                    ui.label("camera_space_convention");
                    ui.monospace(format!("{camera_space_convention:?}"));
                    ui.end_row();

                    if let Some(Intrinsics {
                        intrinsics_matrix,
                        resolution,
                    }) = intrinsics
                    {
                        ui.label("intrinsics matrix");
                        ui_intrinsics_matrix(ui, intrinsics_matrix);
                        ui.end_row();

                        ui.label("resolution");
                        ui.monospace(format!("{resolution:?}"));
                        ui.end_row();
                    }

                    ui.label("target_space");
                    if let Some(target_space) = target_space {
                        ui.monospace(target_space.to_string());
                    }
                    ui.end_row();
                });
        });
    })
    .response
}

fn ui_transform(ui: &mut egui::Ui, transform: &Transform) -> egui::Response {
    match transform {
        Transform::Extrinsics(extrinsics) => ui_extrinsics(ui, extrinsics),
        Transform::Intrinsics(intrinsics) => ui_intrinsics(ui, intrinsics),
    }
}

fn ui_extrinsics(ui: &mut egui::Ui, extrinsics: &Extrinsics) -> egui::Response {
    let Extrinsics {
        rotation,
        position,
        camera_space_convention,
    } = extrinsics;

    ui.vertical(|ui| {
        ui.label("Extrinsics");
        ui.indent("extrinsics", |ui| {
            egui::Grid::new("extrinsics")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("rotation");
                    ui.monospace(format!("{rotation:?}"));
                    ui.end_row();

                    ui.label("position");
                    ui.monospace(format!("{position:?}"));
                    ui.end_row();

                    ui.label("camera_space_convention");
                    ui.monospace(format!("{camera_space_convention:?}"));
                    ui.end_row();
                });
        });
    })
    .response
}

fn ui_intrinsics(ui: &mut egui::Ui, intrinsics: &Intrinsics) -> egui::Response {
    let Intrinsics {
        intrinsics_matrix,
        resolution,
    } = intrinsics;

    ui.vertical(|ui| {
        ui.label("Intrinsics");
        ui.indent("intrinsics", |ui| {
            egui::Grid::new("intrinsics")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("intrinsics matrix");
                    ui_intrinsics_matrix(ui, intrinsics_matrix);
                    ui.end_row();

                    ui.label("resolution");
                    ui.monospace(format!("{resolution:?}"));
                    ui.end_row();
                });
        });
    })
    .response
}

fn ui_intrinsics_matrix(ui: &mut egui::Ui, intrinsics: &[[f32; 3]; 3]) {
    egui::Grid::new("intrinsics").num_columns(3).show(ui, |ui| {
        ui.monospace(intrinsics[0][0].to_string());
        ui.monospace(intrinsics[1][0].to_string());
        ui.monospace(intrinsics[2][0].to_string());
        ui.end_row();

        ui.monospace(intrinsics[0][1].to_string());
        ui.monospace(intrinsics[1][1].to_string());
        ui.monospace(intrinsics[2][1].to_string());
        ui.end_row();

        ui.monospace(intrinsics[0][2].to_string());
        ui.monospace(intrinsics[1][2].to_string());
        ui.monospace(intrinsics[2][2].to_string());
        ui.end_row();
    });
}
