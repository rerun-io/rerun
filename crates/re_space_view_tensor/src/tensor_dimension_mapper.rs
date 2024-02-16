use crate::dimension_mapping::{DimensionMapping, DimensionSelector};
use re_types::datatypes::TensorDimension;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragDropAddress {
    None,
    Width,
    Height,
    Selector(usize),
    NewSelector,
}

impl DragDropAddress {
    fn is_some(&self) -> bool {
        *self != DragDropAddress::None
    }

    fn read_from_address(&self, dimension_mapping: &DimensionMapping) -> Option<usize> {
        match self {
            DragDropAddress::None => unreachable!(),
            DragDropAddress::Width => dimension_mapping.width,
            DragDropAddress::Height => dimension_mapping.height,
            DragDropAddress::Selector(selector_idx) => {
                Some(dimension_mapping.selectors[*selector_idx].dim_idx)
            }
            DragDropAddress::NewSelector => None,
        }
    }

    fn write_to_address(&self, dimension_mapping: &mut DimensionMapping, dim_idx: Option<usize>) {
        match self {
            DragDropAddress::None => unreachable!(),
            DragDropAddress::Width => dimension_mapping.width = dim_idx,
            DragDropAddress::Height => dimension_mapping.height = dim_idx,
            DragDropAddress::Selector(selector_idx) => {
                if let Some(dim_idx) = dim_idx {
                    dimension_mapping.selectors[*selector_idx] = DimensionSelector::new(dim_idx);
                } else {
                    dimension_mapping.selectors.remove(*selector_idx);
                }
            }
            // NewSelector can only be a drop *target*, therefore dim_idx can't be None!
            DragDropAddress::NewSelector => dimension_mapping
                .selectors
                .push(DimensionSelector::new(dim_idx.unwrap())),
        };
    }
}

fn drag_source_ui_id(drag_context_id: egui::Id, dim_idx: usize) -> egui::Id {
    drag_context_id.with("tensor_dimension_ui").with(dim_idx)
}

#[allow(clippy::too_many_arguments)]
fn tensor_dimension_ui(
    ui: &mut egui::Ui,
    drag_context_id: egui::Id,
    bound_dim_idx: Option<usize>,
    location: DragDropAddress,
    shape: &[TensorDimension],
    drag_source: &mut DragDropAddress,
    drop_target: &mut DragDropAddress,
) {
    let frame = egui::Frame::default().inner_margin(4.0);

    let (_response, dropped) = ui.dnd_drop_zone::<DragDropAddress>(frame, |ui| {
        ui.set_min_size(egui::vec2(80., 15.));

        if let Some(dim_idx) = bound_dim_idx {
            let dim = &shape[dim_idx];
            let dim_ui_id = drag_source_ui_id(drag_context_id, dim_idx);

            let label_text = if let Some(dim_name) = dim.name.as_ref() {
                format!("▓ {dim_name} ({})", dim.size)
            } else {
                format!("▓ {dim_idx} ({})", dim.size)
            };

            ui.dnd_drag_source(dim_ui_id, location, |ui| {
                // TODO(emilk): make these buttons respond on hover.
                ui.colored_label(ui.visuals().widgets.inactive.fg_stroke.color, label_text);
            });
        }
    });

    if let Some(dropped) = dropped {
        *drag_source = *dropped;
        *drop_target = location;
    }
}

pub fn dimension_mapping_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    dim_mapping: &mut DimensionMapping,
    shape: &[TensorDimension],
) {
    if !dim_mapping.is_valid(shape.len()) {
        *dim_mapping = DimensionMapping::create(shape);
    }

    let mut drag_source = DragDropAddress::None; // Drag this…
    let mut drop_target = DragDropAddress::None; // …onto this.

    let drag_context_id = ui.id();

    ui.vertical(|ui| {
        ui.vertical(|ui| {
            ui.strong("Image");
            egui::Grid::new("imagegrid").num_columns(2).show(ui, |ui| {
                tensor_dimension_ui(
                    ui,
                    drag_context_id,
                    dim_mapping.width,
                    DragDropAddress::Width,
                    shape,
                    &mut drag_source,
                    &mut drop_target,
                );
                ui.horizontal(|ui| {
                    ui.toggle_value(&mut dim_mapping.invert_width, "Flip");
                    ui.label("width");
                });
                ui.end_row();

                tensor_dimension_ui(
                    ui,
                    drag_context_id,
                    dim_mapping.height,
                    DragDropAddress::Height,
                    shape,
                    &mut drag_source,
                    &mut drop_target,
                );
                ui.horizontal(|ui| {
                    ui.toggle_value(&mut dim_mapping.invert_height, "Flip");
                    ui.label("height");
                });
                ui.end_row();
            });
        });

        ui.add_space(4.0);

        ui.vertical(|ui| {
            ui.strong("Selectors");
            // Use Grid instead of Vertical layout to match styling of the parallel Grid for
            egui::Grid::new("selectiongrid")
                .num_columns(2)
                .show(ui, |ui| {
                    for (selector_idx, selector) in dim_mapping.selectors.iter_mut().enumerate() {
                        tensor_dimension_ui(
                            ui,
                            drag_context_id,
                            Some(selector.dim_idx),
                            DragDropAddress::Selector(selector_idx),
                            shape,
                            &mut drag_source,
                            &mut drop_target,
                        );

                        let response = re_ui.visibility_toggle_button(ui, &mut selector.visible);
                        if selector.visible {
                            response.on_hover_text("Hide dimension slider")
                        } else {
                            response.on_hover_text("Show dimension slider")
                        };
                        ui.end_row();
                    }
                    // Don't expose `NewSelector` for the moment since it doesn't add any value.
                    // We might need it again though if there is a way to park a selector somewhere else than width/height/selector!
                    if false {
                        tensor_dimension_ui(
                            ui,
                            drag_context_id,
                            None,
                            DragDropAddress::NewSelector,
                            shape,
                            &mut drag_source,
                            &mut drop_target,
                        );
                        ui.end_row();
                    }
                });
        });
    });

    // persist drag/drop
    if drag_source.is_some() && drop_target.is_some() {
        let previous_value_source = drag_source.read_from_address(dim_mapping);
        let previous_value_target = drop_target.read_from_address(dim_mapping);
        drag_source.write_to_address(dim_mapping, previous_value_target);
        drop_target.write_to_address(dim_mapping, previous_value_source);
    }
}
