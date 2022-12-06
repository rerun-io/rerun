use std::collections::BTreeMap;

use egui::{color_picker, Vec2};
use re_data_store::{query::visit_type_data_2, FieldName, InstanceId};
use re_log_types::{IndexHash, MsgId};

use crate::{misc::ViewerContext, ui::annotations::auto_color};

pub(crate) fn view_class_description_map(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_id: &InstanceId,
) -> Option<()> {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let store = ctx.log_db.obj_db.store.get(timeline)?;
    let time_query = re_data_store::TimeQuery::LatestAt(ctx.rec_cfg.time_ctrl.time_i64()?);
    let obj_store = store.get(&instance_id.obj_path)?;

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
        .striped(re_ui::ReUi::striped())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto()) // id
        .column(Column::auto().clip(true).at_least(40.0)) // label
        .column(Column::auto()); // color

    table
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.heading("Id");
            });
            header.col(|ui| {
                ui.heading("Label");
            });
            header.col(|ui| {
                ui.heading("Color");
            });
        })
        .body(|mut body| {
            const ROW_HEIGHT: f32 = 18.0;
            for (id, (label, color)) in map {
                body.row(ROW_HEIGHT, |mut row| {
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

    Some(())
}
