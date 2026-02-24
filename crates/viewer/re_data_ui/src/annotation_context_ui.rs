use egui::{NumExt as _, Vec2, color_picker};
use itertools::Itertools as _;
use re_log_types::EntityPath;
use re_sdk_types::components::{self, AnnotationContext};
use re_sdk_types::datatypes::{
    AnnotationInfo, ClassDescription, ClassDescriptionMapElem, KeypointId, KeypointPair,
};
use re_sdk_types::{Component as _, ComponentDescriptor, RowId};
use re_ui::UiExt as _;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::{UiLayout, ViewerContext, auto_color_egui};

use super::DataUi;

impl crate::EntityDataUi for components::ClassId {
    fn entity_data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        _component_descriptor: &ComponentDescriptor,
        _row_id: Option<RowId>,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let annotations = crate::annotations(db, query, entity_path);
        let class = annotations
            .resolved_class_description(Some(*self))
            .class_description;
        if let Some(class) = class {
            let response = ui.horizontal(|ui| {
                // Color first, to keep subsequent rows of the same things aligned
                small_color_ui(ui, &class.info);
                let mut text = format!("{}", self.0);
                if let Some(label) = &class.info.label {
                    text.push(' ');
                    text.push_str(label.as_str());
                }
                ui_layout.label(ui, text);
            });

            let id = self.0;

            if ui_layout.is_single_line() {
                if !class.keypoint_connections.is_empty() || !class.keypoint_annotations.is_empty()
                {
                    response.response.on_hover_ui(|ui| {
                        class_description_ui(ui, UiLayout::Tooltip, class, id);
                    });
                }
            } else {
                ui.separator();
                class_description_ui(ui, ui_layout, class, id);
            }
        } else {
            ui_layout.label(ui, format!("{}", self.0));
        }
    }
}

impl crate::EntityDataUi for components::KeypointId {
    fn entity_data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        _component_descriptor: &ComponentDescriptor,
        _row_id: Option<RowId>,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        if let Some(info) = annotation_info(entity_path, query, db, self.0) {
            ui.horizontal(|ui| {
                // Color first, to keep subsequent rows of the same things aligned
                small_color_ui(ui, &info);
                let mut builder = SyntaxHighlightedBuilder::new();
                builder.append_index(&self.0.to_string());
                if let Some(label) = &info.label {
                    builder.append_string_value(label);
                }

                ui_layout.data_label(ui, builder);
            });
        } else {
            ui_layout.data_label(
                ui,
                SyntaxHighlightedBuilder::new().with_index(&self.0.to_string()),
            );
        }
    }
}

fn annotation_info(
    entity_path: &re_log_types::EntityPath,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    keypoint_id: KeypointId,
) -> Option<AnnotationInfo> {
    // TODO(#3168): this needs to use the index of the keypoint to look up the correct
    // class_id. For now we use `latest_at_component_quiet` to avoid the warning spam.

    // TODO(grtlr): If there's several class ids we have no idea which one to use.
    // This code uses the first one that shows up.
    // We should search instead for a class id that is likely a sibling of the keypoint id.
    let storage_engine = db.storage_engine();
    let store = storage_engine.store();
    let mut possible_class_id_components = store
        .all_components_for_entity(entity_path)?
        .into_iter()
        .filter(|component| {
            let descriptor = store.entity_component_descriptor(entity_path, *component);
            descriptor.is_some_and(|d| d.component_type == Some(components::ClassId::name()))
        });
    let picked_class_id_component = possible_class_id_components.next()?;

    let (_, class_id) = db.latest_at_component_quiet::<components::ClassId>(
        entity_path,
        query,
        picked_class_id_component,
    )?;

    let annotations = crate::annotations(db, query, entity_path);
    let class = annotations.resolved_class_description(Some(class_id));
    class.keypoint_map?.get(&keypoint_id).cloned()
}

impl DataUi for AnnotationContext {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List | UiLayout::Tooltip => {
                let text = if self.0.len() == 1 {
                    let descr = &self.0[0].class_description;

                    format!(
                        "One class containing {} keypoints and {} connections",
                        descr.keypoint_annotations.len(),
                        descr.keypoint_connections.len()
                    )
                } else {
                    format!("{} classes", self.0.len())
                };
                ui_layout.label(ui, text);
            }
            UiLayout::SelectionPanel => {
                ui.vertical(|ui| {
                    ui.maybe_collapsing_header(true, "Classes", true, |ui| {
                        let annotation_infos = self
                            .0
                            .iter()
                            .map(|class| &class.class_description.info)
                            .sorted_by_key(|info| info.id)
                            .collect_vec();
                        annotation_info_table_ui(ui, ui_layout, &annotation_infos);
                    });

                    for ClassDescriptionMapElem {
                        class_id,
                        class_description,
                    } in &self.0
                    {
                        class_description_ui(ui, ui_layout, class_description, *class_id);
                    }
                });
            }
        }
    }
}

fn class_description_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    class: &ClassDescription,
    id: re_sdk_types::datatypes::ClassId,
) {
    if class.keypoint_connections.is_empty() && class.keypoint_annotations.is_empty() {
        return;
    }

    re_tracing::profile_function!();

    let tokens = ui.tokens();

    let use_collapsible = ui_layout == UiLayout::SelectionPanel;

    let table_style = re_ui::TableStyle::Dense;

    let row_height = tokens.table_row_height(table_style);
    if !class.keypoint_annotations.is_empty() {
        ui.maybe_collapsing_header(
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
                    annotation_info_table_ui(ui, ui_layout, &annotation_infos);
                });
            },
        );
    }

    if !class.keypoint_connections.is_empty() {
        ui.maybe_collapsing_header(
            use_collapsible,
            &format!("Keypoint Connections for Class {}", id.0),
            true,
            |ui| {
                use egui_extras::Column;

                let table = ui_layout
                    .table(ui)
                    .id_salt(("keypoints_connections", id))
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().clip(true).at_least(40.0))
                    .column(Column::auto().clip(true).at_least(40.0));
                table
                    .header(tokens.deprecated_table_header_height(), |mut header| {
                        re_ui::DesignTokens::setup_table_header(&mut header);
                        header.col(|ui| {
                            ui.strong("From");
                        });
                        header.col(|ui| {
                            ui.strong("To");
                        });
                    })
                    .body(|mut body| {
                        tokens.setup_table_body(&mut body, table_style);

                        // TODO(jleibs): Helper to do this with caching somewhere
                        let keypoint_map: ahash::HashMap<KeypointId, AnnotationInfo> = {
                            re_tracing::profile_scope!("build_annotation_map");
                            class
                                .keypoint_annotations
                                .iter()
                                .map(|kp| (kp.id.into(), kp.clone()))
                                .collect()
                        };

                        body.rows(row_height, class.keypoint_connections.len(), |mut row| {
                            let pair = &class.keypoint_connections[row.index()];
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
                        });
                    });
            },
        );
    }
}

fn annotation_info_table_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    annotation_infos: &[&AnnotationInfo],
) {
    re_tracing::profile_function!();

    let tokens = ui.tokens();
    let table_style = re_ui::TableStyle::Dense;
    let row_height = tokens.table_row_height(table_style);

    ui.spacing_mut().item_spacing.x = 20.0; // column spacing.

    use egui_extras::Column;

    let table = ui_layout
        .table(ui)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto()) // id
        .column(Column::auto().clip(true).at_least(40.0)) // label
        .column(Column::auto()); // color

    table
        .header(tokens.deprecated_table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
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
            tokens.setup_table_body(&mut body, table_style);

            body.rows(row_height, annotation_infos.len(), |mut row| {
                let info = &annotation_infos[row.index()];
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
            .map_or_else(|| auto_color_egui(info.id), |color| color.into());
        color_picker::show_color(ui, color, size);
        if info.color.is_none() {
            ui.weak("(auto)")
                .on_hover_text("Color chosen automatically, since it was not logged");
        }
    });
}

fn small_color_ui(ui: &mut egui::Ui, info: &AnnotationInfo) {
    let tokens = ui.tokens();
    let size = egui::Vec2::splat(
        tokens
            .table_row_height(re_ui::TableStyle::Dense)
            .at_most(ui.available_height()),
    );

    let color = info
        .color
        .map_or_else(|| auto_color_egui(info.id), |color| color.into());

    let response = color_picker::show_color(ui, color, size);

    if info.color.is_none() {
        response.on_hover_text("Color chosen automatically, since it was not logged");
    }
}
