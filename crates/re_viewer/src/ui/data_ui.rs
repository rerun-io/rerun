use egui::{color_picker, Vec2};

use itertools::Itertools;
use re_data_store::InstanceId;
use re_log_types::context::AnnotationInfo;
use re_log_types::{
    context, AnnotationContext, Arrow3D, ArrowMsg, BeginRecordingMsg, Data, DataMsg, DataPath,
    DataVec, LogMsg, LoggedData, MsgId, ObjPath, ObjectType, PathOp, PathOpMsg, Pinhole,
    RecordingInfo, Rigid3, TimePoint, Transform, TypeMsg, ViewCoordinates,
};

use crate::misc::ViewerContext;
use crate::ui::annotations::auto_color;
use crate::ui::view_2d::view_class_description_map;

use super::{annotations::AnnotationMap, Preview};

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
    match ctx
        .log_db
        .obj_db
        .types
        .get(instance_id.obj_path.obj_type_path())
    {
        Some(ObjectType::ClassDescription) => view_class_description_map(ctx, ui, instance_id),
        _ => view_instance_generic(ctx, ui, instance_id, preview),
    }
}

pub(crate) fn view_instance_generic(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_id: &InstanceId,
    preview: Preview,
) -> Option<()> {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let store = ctx.log_db.obj_db.store.get(timeline)?;
    let time_query = re_data_store::TimeQuery::LatestAt(ctx.rec_cfg.time_ctrl.time_i64()?);
    let obj_store = store.get(&instance_id.obj_path)?;

    let mut class_id = None;
    let mut keypoint_id = None;

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
                    Ok((_, data_vec)) => {
                        if data_vec.len() == 1 {
                            let data = data_vec.last().unwrap();
                            if field_name.as_str() == "class_id" {
                                if let Data::I32(id) = data {
                                    class_id = Some(context::ClassId(id as _));
                                }
                            }
                            if field_name.as_str() == "keypoint_id" {
                                if let Data::I32(id) = data {
                                    keypoint_id = Some(context::KeypointId(id as _));
                                }
                            }
                            crate::data_ui::ui_data(ctx, ui, &data, preview);
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

    // If we have a class id, show some information about the resolved style!
    if let Some(class_id) = class_id {
        ui.separator();

        if let Some((data_path, annotations)) =
            AnnotationMap::find_associated(ctx, instance_id.obj_path.clone())
        {
            ctx.data_path_button_to(
                ui,
                format!("Annotation Context at {}", data_path.obj_path),
                &data_path,
            );
            egui::Grid::new("class_description")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    if let Some(class_description) = annotations.context.class_map.get(&class_id) {
                        let class_annotation = &class_description.info;
                        let mut keypoint_annotation = None;

                        if let Some(keypoint_id) = keypoint_id {
                            keypoint_annotation = class_description.keypoint_map.get(&keypoint_id);
                            if keypoint_annotation.is_none() {
                                ui.label(ctx.re_ui.warning_text(format!(
                                    "unknown keypoint_id {}",
                                    keypoint_id.0
                                )));
                            }
                        }

                        if let Some(label) = keypoint_annotation
                            .and_then(|a| a.label.as_ref())
                            .or(class_annotation.label.as_ref())
                        {
                            ui.label("label");
                            ui.label(label.as_ref());
                            ui.end_row();
                        }
                        if let Some(color) = keypoint_annotation
                            .and_then(|a| a.color.as_ref())
                            .or(class_annotation.color.as_ref())
                        {
                            ui.label("color");
                            ui_color_field(ui, color);
                            ui.end_row();
                        }
                    } else {
                        ui.label(
                            ctx.re_ui
                                .warning_text(format!("unknown class_id {}", class_id.0)),
                        );
                    }
                });
        } else {
            ui.label(
                ctx.re_ui
                    .warning_text("class_id specified, but no annotation context found"),
            );
        }
    }

    Some(())
}

pub(crate) fn view_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    data_path: &DataPath,
) -> Option<()> {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let time_query = re_data_store::TimeQuery::LatestAt(ctx.rec_cfg.time_ctrl.time_i64()?);

    match ctx
        .log_db
        .obj_db
        .store
        .query_data_path(timeline, &time_query, data_path)?
    {
        Ok((_, data_vec)) => {
            if data_vec.len() == 1 {
                let data = data_vec.last().unwrap();
                show_detailed_data(ctx, ui, &data);
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

pub(crate) fn show_detailed_data(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, data: &Data) {
    if let Data::Tensor(tensor) = data {
        crate::ui::view_2d::show_tensor(ctx, ui, tensor);
    } else {
        crate::data_ui::ui_data(ctx, ui, data, Preview::Medium);
    }
}

pub(crate) fn show_detailed_data_msg(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &DataMsg,
) {
    let DataMsg {
        msg_id: _,
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
                crate::data_ui::ui_logged_data(ctx, ui, data, Preview::Medium);
                ui.end_row();
            }
        });

    if let LoggedData::Single(Data::Tensor(tensor)) = &msg.data {
        crate::ui::view_2d::show_tensor(ctx, ui, tensor);
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
        LogMsg::PathOpMsg(msg) => {
            show_path_op_msg(ctx, ui, msg);
        }
        LogMsg::ArrowMsg(msg) => {
            show_arrow_msg(ctx, ui, msg, preview);
        }
    }
}

pub(crate) fn show_begin_recording_msg(ui: &mut egui::Ui, msg: &BeginRecordingMsg) {
    ui.code("BeginRecordingMsg");
    let BeginRecordingMsg { msg_id: _, info } = msg;
    let RecordingInfo {
        application_id,
        recording_id,
        started,
        recording_source,
    } = info;

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("application_id:");
            ui.label(application_id.to_string());
            ui.end_row();

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
        msg_id: _,
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
            ui_logged_data(ctx, ui, data, preview);
            ui.end_row();
        });
}

pub(crate) fn show_path_op_msg(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, msg: &PathOpMsg) {
    let PathOpMsg {
        msg_id: _,
        time_point,
        path_op,
    } = msg;

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("time_point:");
            ui_time_point(ctx, ui, time_point);
            ui.end_row();

            ui.monospace("path_op:");
            ui_path_op(ctx, ui, path_op);
            ui.end_row();
        });
}

pub(crate) fn show_arrow_msg(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &ArrowMsg,
    preview: Preview,
) {
    let ArrowMsg {
        msg_id,
        schema: _,
        chunk: _,
    } = msg;

    // TODO(jleibs): Better ArrowMsg view
    ui_logged_arrow_data(ctx, ui, msg_id, msg, preview);
}

pub(crate) fn ui_time_point(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    time_point: &TimePoint,
) {
    ui.vertical(|ui| {
        egui::Grid::new("time_point").num_columns(2).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (timeline, value) in time_point.iter() {
                ctx.timeline_button(ui, timeline);
                ui.label(": ");
                ctx.time_button(ui, timeline, *value);
                ui.end_row();
            }
        });
    });
}

pub(crate) fn ui_logged_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    data: &LoggedData,
    preview: Preview,
) -> egui::Response {
    match data {
        LoggedData::Null(data_type) => ui.label(format!("null: {:?}", data_type)),
        LoggedData::Batch { data, .. } => ui.label(format!("batch: {:?}", data)),
        LoggedData::Single(data) => ui_data(ctx, ui, data, preview),
        LoggedData::BatchSplat(data) => {
            ui.horizontal(|ui| {
                ui.label("Batch Splat:");
                ui_data(ctx, ui, data, preview)
            })
            .response
        }
    }
}

// TODO(jleibs): Better ArrowMsg view
pub(crate) fn ui_logged_arrow_data(
    _ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    _msg_id: &MsgId,
    msg: &ArrowMsg,
    _preview: Preview,
) -> egui::Response {
    // TODO(john): more handling
    //let arr = msg.to_arrow_array();
    ui.label(format!("Arrow Payload: ({:?})", msg.schema))
}

pub(crate) fn ui_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    data: &Data,
    preview: Preview,
) -> egui::Response {
    match data {
        Data::Bool(value) => ui.label(value.to_string()),
        Data::I32(value) => ui.label(value.to_string()),
        Data::F32(value) => ui.label(value.to_string()),
        Data::F64(value) => ui.label(value.to_string()),
        Data::Color(value) => ui_color_field(ui, value),
        Data::String(string) => ui.label(format!("{string:?}")),

        Data::Vec2([x, y]) => ui.label(format!("[{x:.1}, {y:.1}]")),
        Data::BBox2D(bbox) => ui.label(format!(
            "BBox2D(min: [{:.1} {:.1}], max: [{:.1} {:.1}])",
            bbox.min[0], bbox.min[1], bbox.max[0], bbox.max[1]
        )),

        Data::Vec3([x, y, z]) => ui.label(format!("[{x:.3}, {y:.3}, {z:.3}]")),
        Data::Box3(_) => ui.label("3D box"),
        Data::Mesh3D(_) => ui.label("3D mesh"),
        Data::Arrow3D(Arrow3D { origin, vector }) => {
            let &[x, y, z] = origin;
            let &[v0, v1, v2] = vector;
            ui.label(format!(
                "Arrow3D(origin: [{x:.1},{y:.1},{z:.1}], vector: [{v0:.1},{v1:.1},{v2:.1}])"
            ))
        }
        Data::Transform(transform) => match preview {
            Preview::Small | Preview::Specific(_) => ui.monospace("Transform"),
            Preview::Medium => ui_transform(ui, transform),
        },
        Data::ViewCoordinates(coordinates) => match preview {
            Preview::Small | Preview::Specific(_) => {
                ui.label(format!("ViewCoordinates: {}", coordinates.describe()))
            }
            Preview::Medium => ui_view_coordinates(ui, coordinates),
        },
        Data::AnnotationContext(context) => match preview {
            Preview::Small | Preview::Specific(_) => ui.monospace("AnnotationContext"),
            Preview::Medium => ui_annotation_context(ui, context),
        },
        Data::Tensor(tensor) => {
            let tensor_view = ctx.cache.image.get_view(tensor, ctx.render_ctx);

            ui.horizontal_centered(|ui| {
                let max_width = match preview {
                    Preview::Small => 32.0,
                    Preview::Medium => 128.0,
                    Preview::Specific(height) => height,
                };

                tensor_view
                    .retained_img
                    .show_max_size(ui, Vec2::new(4.0 * max_width, max_width))
                    .on_hover_ui(|ui| {
                        tensor_view
                            .retained_img
                            .show_max_size(ui, Vec2::splat(400.0));
                    });

                ui.vertical(|ui| {
                    ui.set_min_width(100.0);
                    ui.label(format!("dtype: {:?}", tensor.dtype));
                    ui.label(format!("shape: {:?}", tensor.shape));
                });
            })
            .response
        }

        Data::ObjPath(obj_path) => ctx.obj_path_button(ui, obj_path),

        Data::DataVec(data_vec) => ui_data_vec(ui, data_vec),
    }
}

fn ui_color_field(ui: &mut egui::Ui, value: &[u8; 4]) -> egui::Response {
    let [r, g, b, a] = value;
    let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
    let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
    ui.painter().rect_stroke(
        response.rect,
        1.0,
        ui.visuals().widgets.noninteractive.fg_stroke,
    );
    response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
}

pub(crate) fn ui_data_vec(ui: &mut egui::Ui, data_vec: &DataVec) -> egui::Response {
    ui.label(format!(
        "{} x {:?}",
        data_vec.len(),
        data_vec.element_data_type(),
    ))
}

fn ui_transform(ui: &mut egui::Ui, transform: &Transform) -> egui::Response {
    match transform {
        Transform::Unknown => ui.label("Unknown"),
        Transform::Rigid3(rigid3) => ui_rigid3(ui, rigid3),
        Transform::Pinhole(pinhole) => ui_pinhole(ui, pinhole),
    }
}

fn ui_view_coordinates(ui: &mut egui::Ui, system: &ViewCoordinates) -> egui::Response {
    ui.label(system.describe())
}

const ROW_HEIGHT: f32 = 18.0;
const TABLE_SCROLL_AREA_HEIGHT: f32 = 500.0; // add scroll-bars when we get to this height

fn ui_annotation_info_table<'a>(
    ui: &mut egui::Ui,
    annotation_infos: impl Iterator<Item = &'a AnnotationInfo>,
) {
    ui.spacing_mut().item_spacing.x = 20.0; // column spacing.

    use egui_extras::{Column, TableBuilder};

    let table = TableBuilder::new(ui)
        .striped(true)
        .min_scrolled_height(TABLE_SCROLL_AREA_HEIGHT)
        .max_scroll_height(TABLE_SCROLL_AREA_HEIGHT)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto()) // id
        .column(Column::auto().clip(true).at_least(40.0)) // label
        .column(Column::auto()); // color

    table
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Id");
            });
            header.col(|ui| {
                ui.strong("Label");
            });
            header.col(|ui| {
                ui.strong("Color");
            });
        })
        .body(|mut body| {
            for info in annotation_infos {
                body.row(ROW_HEIGHT, |mut row| {
                    row.col(|ui| {
                        ui.label(info.id.to_string());
                    });
                    row.col(|ui| {
                        let label = if let Some(label) = &info.label {
                            label.as_str()
                        } else {
                            ""
                        };
                        ui.label(label);
                    });
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let color = info.color.unwrap_or_else(|| auto_color(info.id));
                            let color = egui::Color32::from_rgb(color[0], color[1], color[2]);
                            color_picker::show_color(ui, color, Vec2::new(64.0, ROW_HEIGHT));
                            if info.color.is_none() {
                                ui.weak("(auto)").on_hover_text(
                                    "Color chosen automatically, since it was not logged.",
                                );
                            }
                        });
                    });
                });
            }
        });
}

fn ui_annotation_context(ui: &mut egui::Ui, context: &AnnotationContext) -> egui::Response {
    ui.vertical(|ui| {
        ui_annotation_info_table(
            ui,
            context
                .class_map
                .iter()
                .map(|(_, class)| &class.info)
                .sorted_by_key(|info| info.id),
        );

        for (id, class) in &context.class_map {
            if class.keypoint_connections.is_empty() && class.keypoint_map.is_empty() {
                continue;
            }

            ui.separator();
            ui.heading(format!("Keypoints for Class {}", id.0));

            if !class.keypoint_connections.is_empty() {
                ui.add_space(8.0);
                ui.heading("Keypoints Annotations");
                ui.push_id(format!("keypoint_annotations_{}", id.0), |ui| {
                    ui_annotation_info_table(
                        ui,
                        class
                            .keypoint_map
                            .values()
                            .sorted_by_key(|annotation| annotation.id),
                    );
                });
            }

            if !class.keypoint_connections.is_empty() {
                ui.add_space(8.0);
                ui.heading("Keypoint Connections");
                ui.push_id(format!("keypoints_connections_{}", id.0), |ui| {
                    use egui_extras::{Column, TableBuilder};

                    let table = TableBuilder::new(ui)
                        .striped(true)
                        .min_scrolled_height(TABLE_SCROLL_AREA_HEIGHT)
                        .max_scroll_height(TABLE_SCROLL_AREA_HEIGHT)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::auto().clip(true).at_least(40.0))
                        .column(Column::auto().clip(true).at_least(40.0));
                    table
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.strong("From");
                            });
                            header.col(|ui| {
                                ui.strong("To");
                            });
                        })
                        .body(|mut body| {
                            for (from, to) in &class.keypoint_connections {
                                body.row(ROW_HEIGHT, |mut row| {
                                    for id in [from, to] {
                                        row.col(|ui| {
                                            ui.label(
                                                class
                                                    .keypoint_map
                                                    .get(id)
                                                    .and_then(|info| info.label.as_ref())
                                                    .map_or_else(
                                                        || format!("id {:?}", id),
                                                        |label| String::clone(label),
                                                    ),
                                            );
                                        });
                                    }
                                });
                            }
                        });
                });
            }
        }
    })
    .response
}

fn ui_rigid3(ui: &mut egui::Ui, rigid3: &Rigid3) -> egui::Response {
    let pose = rigid3.parent_from_child(); // TODO(emilk): which one to show?
    let rotation = pose.rotation();
    let translation = pose.translation();

    ui.vertical(|ui| {
        ui.label("Rigid3");
        ui.indent("rigid3", |ui| {
            egui::Grid::new("rigid3")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("rotation");
                    ui.monospace(format!("{rotation:?}"));
                    ui.end_row();

                    ui.label("translation");
                    ui.monospace(format!("{translation:?}"));
                    ui.end_row();
                });
        });
    })
    .response
}

fn ui_pinhole(ui: &mut egui::Ui, pinhole: &Pinhole) -> egui::Response {
    let Pinhole {
        image_from_cam: image_from_view,
        resolution,
    } = pinhole;

    ui.vertical(|ui| {
        ui.label("Pinhole");
        ui.indent("pinole", |ui| {
            egui::Grid::new("pinole")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("image from view");
                    ui_mat3(ui, image_from_view);
                    ui.end_row();

                    ui.label("resolution");
                    ui.monospace(format!("{resolution:?}"));
                    ui.end_row();
                });
        });
    })
    .response
}

fn ui_mat3(ui: &mut egui::Ui, mat: &[[f32; 3]; 3]) {
    egui::Grid::new("mat3").num_columns(3).show(ui, |ui| {
        ui.monospace(mat[0][0].to_string());
        ui.monospace(mat[1][0].to_string());
        ui.monospace(mat[2][0].to_string());
        ui.end_row();

        ui.monospace(mat[0][1].to_string());
        ui.monospace(mat[1][1].to_string());
        ui.monospace(mat[2][1].to_string());
        ui.end_row();

        ui.monospace(mat[0][2].to_string());
        ui.monospace(mat[1][2].to_string());
        ui.monospace(mat[2][2].to_string());
        ui.end_row();
    });
}

pub(crate) fn ui_path_op(
    _ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    path_op: &PathOp,
) -> egui::Response {
    match path_op {
        PathOp::ClearFields(obj_path) => ui.label(format!("ClearFields: {obj_path}")),
        PathOp::ClearRecursive(obj_path) => ui.label(format!("ClearRecursive: {obj_path}")),
    }
}
