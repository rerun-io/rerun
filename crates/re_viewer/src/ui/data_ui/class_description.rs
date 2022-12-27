use std::collections::BTreeMap;

use egui::{color_picker, Sense, Vec2};
use re_data_store::{query::visit_type_data_2, FieldName, InstanceId};
use re_log_types::{IndexHash, MsgId};

use crate::{misc::ViewerContext, ui::annotations::auto_color};

pub(crate) fn class_description_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_id: &InstanceId,
) -> egui::Response {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let Some(store) = ctx.log_db.obj_db.store.get(timeline) else {
        return ui.label(ctx.re_ui.error_text("No store with timeline {timeline}."))
    };
    let Some(time_i64) = ctx.rec_cfg.time_ctrl.time_i64() else {
        return ui.label(ctx.re_ui.error_text("No current time."))
    };
    let time_query = re_data_store::TimeQuery::LatestAt(time_i64);
    let Some(obj_store) = store.get(&instance_id.obj_path) else {
        return ui.label(ctx.re_ui.error_text(format!("No object at path {}", instance_id.obj_path)))
    };

    // TODO(jleibs) This should really used a shared implementation with objects.rs
    let mut map = BTreeMap::<i32, (Option<&str>, egui::Color32)>::default();

    visit_type_data_2(
        obj_store,
        &FieldName::from("id"),
        &time_query,
        ("label", "color"),
        |_instance_index: Option<&IndexHash>,
         _time,
         _msg_id: &MsgId,
         id: &i32,
         label: Option<&String>,
         color: Option<&[u8; 4]>| {
            let val = u16::try_from(*id % (u16::MAX as i32)).unwrap();
            let color = *color.unwrap_or(&auto_color(val));
            map.insert(
                *id,
                (
                    label.map(|s| s.as_str()),
                    egui::Color32::from_rgb(color[0], color[1], color[2]),
                ),
            );
        },
    );

    use egui_extras::{Column, TableBuilder};

    let table = TableBuilder::new(ui)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto()) // id
        .column(Column::auto().clip(true).at_least(40.0)) // label
        .column(Column::auto()); // color

    table
        .header(re_ui::ReUi::table_header_height(), |mut header| {
            re_ui::ReUi::setup_table_header(&mut header);
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
            re_ui::ReUi::setup_table_body(&mut body);

            let row_height = re_ui::ReUi::table_line_height();
            for (id, (label, color)) in map {
                body.row(row_height, |mut row| {
                    row.col(|ui| {
                        ui.label(id.to_string());
                    });
                    row.col(|ui| {
                        ui.label(label.unwrap_or(""));
                    });
                    row.col(|ui| {
                        color_picker::show_color(ui, color, Vec2::splat(64.0));
                    });
                });
            }
        });

    //TODO(john) figure out how to do this better, or patch TableBuilder to provide response
    ui.allocate_response(Vec2::ZERO, Sense::hover())
}
