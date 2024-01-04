use egui::{color_picker, Vec2};
use itertools::Itertools;

use re_types::components::AnnotationContext;
use re_types::datatypes::{
    AnnotationInfo, ClassDescription, ClassDescriptionMapElem, KeypointId, KeypointPair,
};
use re_viewer_context::{auto_color, UiVerbosity, ViewerContext};

use super::{table_for_verbosity, DataUi};

impl crate::EntityDataUi for re_types::components::ClassId {
    fn entity_data_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: re_viewer_context::UiVerbosity,
        entity_path: &re_log_types::EntityPath,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let annotations = crate::annotations(ctx, query, entity_path);
        let class = annotations
            .resolved_class_description(Some(*self))
            .class_description;
        if let Some(class) = class {
            let response = ui.horizontal(|ui| {
                // Color first, to keep subsequent rows of the same things aligned
                small_color_ui(ui, &class.info);
                ui.label(format!("{}", self.0));
                if let Some(label) = &class.info.label {
                    ui.label(label.as_str());
                }
            });

            let id = self.0;
            match verbosity {
                UiVerbosity::Small => {
                    if !class.keypoint_connections.is_empty()
                        || !class.keypoint_annotations.is_empty()
                    {
                        response.response.on_hover_ui(|ui| {
                            class_description_ui(ctx, ui, verbosity, class, id);
                        });
                    }
                }
                UiVerbosity::Reduced | UiVerbosity::Full | UiVerbosity::LimitHeight => {
                    ui.separator();
                    class_description_ui(ctx, ui, verbosity, class, id);
                }
            }
        } else {
            ui.label(format!("{}", self.0));
        }
    }
}

impl crate::EntityDataUi for re_types::components::KeypointId {
    fn entity_data_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: re_viewer_context::UiVerbosity,
        entity_path: &re_log_types::EntityPath,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        if let Some(info) = annotation_info(ctx, entity_path, query, self.0) {
            ui.horizontal(|ui| {
                // Color first, to keep subsequent rows of the same things aligned
                small_color_ui(ui, &info);
                ui.label(format!("{}", self.0));
                if let Some(label) = &info.label {
                    ui.label(label.as_str());
                }
            });
        } else {
            ui.label(format!("{}", self.0));
        }
    }
}

fn annotation_info(
    ctx: &re_viewer_context::ViewerContext<'_>,
    entity_path: &re_log_types::EntityPath,
    query: &re_arrow_store::LatestAtQuery,
    keypoint_id: KeypointId,
) -> Option<AnnotationInfo> {
    let class_id = ctx
        .entity_db
        .store()
        .query_latest_component::<re_types::components::ClassId>(entity_path, query)?;
    let annotations = crate::annotations(ctx, query, entity_path);
    let class = annotations.resolved_class_description(Some(*class_id));
    class.keypoint_map?.get(&keypoint_id).cloned()
}

impl DataUi for AnnotationContext {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                if self.0.len() == 1 {
                    let descr = &self.0[0].class_description;
                    ui.label(format!("AnnotationContext with one class containing {} keypoints and {} connections", descr.keypoint_annotations.len(), descr.keypoint_connections.len()));
                } else {
                    ui.label(format!("AnnotationContext with {} classes", self.0.len()));
                }
            }
            UiVerbosity::LimitHeight | UiVerbosity::Full => {
                ui.vertical(|ui| {
                    ctx.re_ui
                        .maybe_collapsing_header(ui, true, "Classes", true, |ui| {
                            let annotation_infos = self
                                .0
                                .iter()
                                .map(|class| &class.class_description.info)
                                .sorted_by_key(|info| info.id)
                                .collect_vec();
                            annotation_info_table_ui(ui, verbosity, &annotation_infos);
                        });

                    for ClassDescriptionMapElem {
                        class_id,
                        class_description,
                    } in &self.0
                    {
                        class_description_ui(ctx, ui, verbosity, class_description, *class_id);
                    }
                });
            }
        }
    }
}

fn class_description_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    mut verbosity: UiVerbosity,
    class: &ClassDescription,
    id: re_types::datatypes::ClassId,
) {
    if class.keypoint_connections.is_empty() && class.keypoint_annotations.is_empty() {
        return;
    }

    re_tracing::profile_function!();

    let use_collapsible = verbosity == UiVerbosity::LimitHeight || verbosity == UiVerbosity::Full;

    // We use collapsible header as a means for the user to limit the height, so the annotation info
    // tables can be fully unrolled.
    if verbosity == UiVerbosity::LimitHeight {
        verbosity = UiVerbosity::Full;
    }

    let row_height = re_ui::ReUi::table_line_height();
    if !class.keypoint_annotations.is_empty() {
        ctx.re_ui.maybe_collapsing_header(
            ui,
            use_collapsible,
            &format!("Keypoints Annotation for Class {}", id.0),
            true,
            |ui| {
                let annotation_infos = class
                    .keypoint_annotations
                    .iter()
                    .sorted_by_key(|annotation| annotation.id)
                    .collect_vec();
                ui.push_id(format!("keypoint_annotations_{}", id.0), |ui| {
                    annotation_info_table_ui(ui, verbosity, &annotation_infos);
                });
            },
        );
    }

    if !class.keypoint_connections.is_empty() {
        ctx.re_ui.maybe_collapsing_header(
            ui,
            use_collapsible,
            &format!("Keypoint Connections for Class {}", id.0),
            true,
            |ui| {
                ui.push_id(format!("keypoints_connections_{}", id.0), |ui| {
                    use egui_extras::Column;

                    let table = table_for_verbosity(verbosity, ui)
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

                            // TODO(jleibs): Helper to do this with caching somewhere
                            let keypoint_map: ahash::HashMap<KeypointId, AnnotationInfo> = {
                                re_tracing::profile_scope!("build_annotation_map");
                                class
                                    .keypoint_annotations
                                    .iter()
                                    .map(|kp| (kp.id.into(), kp.clone()))
                                    .collect()
                            };

                            body.rows(
                                row_height,
                                class.keypoint_connections.len(),
                                |row_idx, mut row| {
                                    let pair = &class.keypoint_connections[row_idx];
                                    let KeypointPair {
                                        keypoint0,
                                        keypoint1,
                                    } = pair;

                                    for id in [keypoint0, keypoint1] {
                                        row.col(|ui| {
                                            ui.label(
                                                keypoint_map
                                                    .get(id)
                                                    .and_then(|info| info.label.as_ref())
                                                    .map_or_else(
                                                        || format!("id {}", id.0),
                                                        |label| label.to_string(),
                                                    ),
                                            );
                                        });
                                    }
                                },
                            );
                        });
                });
            },
        );
    }
}

fn annotation_info_table_ui(
    ui: &mut egui::Ui,
    verbosity: UiVerbosity,
    annotation_infos: &[&AnnotationInfo],
) {
    re_tracing::profile_function!();

    let row_height = re_ui::ReUi::table_line_height();

    ui.spacing_mut().item_spacing.x = 20.0; // column spacing.

    use egui_extras::Column;

    let table = table_for_verbosity(verbosity, ui)
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

            body.rows(row_height, annotation_infos.len(), |row_idx, mut row| {
                let info = &annotation_infos[row_idx];
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
                    color_ui(ui, info, Vec2::new(64.0, row_height));
                });
            });
        });
}

fn color_ui(ui: &mut egui::Ui, info: &AnnotationInfo, size: Vec2) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let color = info
            .color
            .map_or_else(|| auto_color(info.id), |color| color.into());
        color_picker::show_color(ui, color, size);
        if info.color.is_none() {
            ui.weak("(auto)")
                .on_hover_text("Color chosen automatically, since it was not logged");
        }
    });
}

fn small_color_ui(ui: &mut egui::Ui, info: &AnnotationInfo) {
    let size = egui::Vec2::splat(re_ui::ReUi::table_line_height());

    let color = info
        .color
        .map_or_else(|| auto_color(info.id), |color| color.into());

    let response = color_picker::show_color(ui, color, size);

    if info.color.is_none() {
        response.on_hover_text("Color chosen automatically, since it was not logged");
    }
}
