use egui::{color_picker, Vec2};
use itertools::Itertools;
use re_log_types::{context::AnnotationInfo, AnnotationContext};

use crate::ui::{annotations::auto_color, UiVerbosity};

use super::DataUi;

const TABLE_SCROLL_AREA_HEIGHT: f32 = 500.0; // add scroll-bars when we get to this height

impl DataUi for AnnotationContext {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: crate::ui::UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
                ui.label(format!(
                    "AnnotationContext with {} classes",
                    self.class_map.len()
                ));
            }
            UiVerbosity::Large => {
                let row_height = re_ui::ReUi::table_line_height();
                ui.vertical(|ui| {
                    annotation_info_table_ui(
                        ui,
                        self.class_map.values().map(|class| &class.info)
                            .sorted_by_key(|info| info.id),
                    );

                    for (id, class) in &self.class_map {
                        if class.keypoint_connections.is_empty() && class.keypoint_map.is_empty() {
                            continue;
                        }

                        ui.separator();
                        ui.strong(format!("Keypoints for Class {}", id.0));

                        if !class.keypoint_connections.is_empty() {
                            ui.add_space(8.0);
                            ui.strong("Keypoints Annotations");
                            ui.push_id(format!("keypoint_annotations_{}", id.0), |ui| {
                                annotation_info_table_ui(
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
                            ui.strong("Keypoint Connections");
                            ui.push_id(format!("keypoints_connections_{}", id.0), |ui| {
                                use egui_extras::{Column, TableBuilder};

                                let table = TableBuilder::new(ui)
                                    .min_scrolled_height(TABLE_SCROLL_AREA_HEIGHT)
                                    .max_scroll_height(TABLE_SCROLL_AREA_HEIGHT)
                                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                    .column(Column::auto().clip(true).at_least(40.0))
                                    .column(Column::auto().clip(true).at_least(40.0));
                                table
                                    .header(re_ui::ReUi::table_header_height(), |mut header| {
                                        re_ui::ReUi::setup_table_header(&mut header);
                                        header.col(|ui| {
                                            ui.strong("From");
                                        });
                                        header.col(|ui| {
                                            ui.strong("To");
                                        });
                                    })
                                    .body(|mut body| {
                                        re_ui::ReUi::setup_table_body(&mut body);

                                        for (from, to) in &class.keypoint_connections {
                                            body.row(row_height, |mut row| {
                                                for id in [from, to] {
                                                    row.col(|ui| {
                                                        ui.label(
                                                            class
                                                                .keypoint_map
                                                                .get(id)
                                                                .and_then(|info| {
                                                                    info.label.as_ref()
                                                                })
                                                                .map_or_else(
                                                                    || format!("id {id:?}"),
                                                                    |label| label.0.clone(),
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
                });
            }
        }
    }
}

fn annotation_info_table_ui<'a>(
    ui: &mut egui::Ui,
    annotation_infos: impl Iterator<Item = &'a AnnotationInfo>,
) {
    let row_height = re_ui::ReUi::table_line_height();

    ui.spacing_mut().item_spacing.x = 20.0; // column spacing.

    use egui_extras::{Column, TableBuilder};

    let table = TableBuilder::new(ui)
        .min_scrolled_height(TABLE_SCROLL_AREA_HEIGHT)
        .max_scroll_height(TABLE_SCROLL_AREA_HEIGHT)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto()) // id
        .column(Column::auto().clip(true).at_least(40.0)) // label
        .column(Column::auto()); // color

    table
        .header(re_ui::ReUi::table_header_height(), |mut header| {
            re_ui::ReUi::setup_table_header(&mut header);
            header.col(|ui| {
                ui.strong("Class Id");
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

            for info in annotation_infos {
                body.row(row_height, |mut row| {
                    row.col(|ui| {
                        ui.label(info.id.to_string());
                    });
                    row.col(|ui| {
                        let label = if let Some(label) = &info.label {
                            label.0.as_str()
                        } else {
                            ""
                        };
                        ui.label(label);
                    });
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let color = info
                                .color
                                .map_or_else(|| auto_color(info.id), |color| color.into());
                            color_picker::show_color(ui, color, Vec2::new(64.0, row_height));
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
