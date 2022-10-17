use egui::{color_picker, Vec2};
use egui_extras::{Size, TableBuilder};
use nohash_hasher::IntMap;
use re_data_store::{query::visit_type_data_2, FieldName, InstanceId};
use re_log_types::{IndexHash, MsgId};

use crate::misc::ViewerContext;

pub(crate) fn view_segmentation_map(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_id: &InstanceId,
) -> Option<()> {
    let store = ctx
        .log_db
        .obj_db
        .store
        .get(ctx.rec_cfg.time_ctrl.timeline())?;
    let time_query = ctx.rec_cfg.time_ctrl.time_query()?;
    let obj_store = store.get(&instance_id.obj_path)?;

    let mut map = IntMap::<i32, (Option<&str>, egui::Color32)>::default();

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
            let color = color.unwrap_or(&[0, 0, 0, 0]);
            map.insert(
                *id,
                (
                    label.map(|s| s.as_str()),
                    egui::Color32::from_rgb(color[0], color[1], color[2]),
                ),
            );
        },
    );

    let table = TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Size::initial(60.0).at_least(40.0))
        .column(Size::initial(60.0).at_least(40.0))
        .column(Size::remainder().at_least(60.0));

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
