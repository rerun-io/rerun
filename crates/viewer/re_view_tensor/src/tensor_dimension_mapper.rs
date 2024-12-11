use re_types::{
    blueprint::archetypes::TensorSliceSelection, datatypes::TensorDimensionIndexSelection,
};
use re_ui::UiExt as _;
use re_viewer_context::ViewerContext;
use re_viewport_blueprint::ViewProperty;

use crate::TensorDimension;

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
        *self != Self::None
    }

    fn read_from_address(
        &self,
        slice_selection: &TensorSliceSelection,
        shape: &[TensorDimension],
    ) -> Option<TensorDimensionIndexSelection> {
        match self {
            Self::None => unreachable!(),
            Self::Width => slice_selection
                .width
                .map(|w| TensorDimensionIndexSelection {
                    dimension: w.dimension,
                    index: shape[w.dimension as usize].size / 2, // Select middle if this becomes index fixed.
                }),
            Self::Height => slice_selection
                .height
                .map(|h| TensorDimensionIndexSelection {
                    dimension: h.dimension,
                    index: shape[h.dimension as usize].size / 2, // Select middle if this becomes index fixed.
                }),
            #[allow(clippy::unwrap_used)]
            Self::Selector(selector_idx) => {
                Some(slice_selection.indices.as_ref().unwrap()[*selector_idx].0)
            }
            Self::NewSelector => None,
        }
    }

    fn write_to_address(
        &self,
        ctx: &ViewerContext<'_>,
        slice_selection: &TensorSliceSelection,
        slice_property: &ViewProperty,
        new_selection: Option<TensorDimensionIndexSelection>,
    ) {
        match self {
            Self::None => unreachable!(),
            Self::Width => {
                let width = new_selection.map(|new_selection| {
                    let mut width = slice_selection.width.unwrap_or_default();
                    width.dimension = new_selection.dimension;
                    width
                });
                slice_property.save_blueprint_component(ctx, &width);
            }
            Self::Height => {
                let height = new_selection.map(|new_selection| {
                    let mut height = slice_selection.height.unwrap_or_default();
                    height.dimension = new_selection.dimension;
                    height
                });
                slice_property.save_blueprint_component(ctx, &height);
            }
            Self::Selector(selector_idx) => {
                let mut indices = slice_selection.indices.clone().unwrap_or_default();
                let mut slider = slice_selection.slider.clone().unwrap_or_default();
                if let Some(new_selection) = new_selection {
                    indices[*selector_idx] = new_selection.into();
                    slider.push(new_selection.dimension.into()); // Enable slider by default.
                } else {
                    let removed_dim = indices[*selector_idx].dimension;
                    slider.retain(|s| s.dimension != removed_dim); // purge slider if there was any.
                    indices.remove(*selector_idx);
                }
                slice_property.save_blueprint_component(ctx, &indices);
                slice_property.save_blueprint_component(ctx, &slider);
            }
            Self::NewSelector => {
                // NewSelector can only be a drop *target*, therefore dim_idx can't be None!
                if let Some(new_selection) = new_selection {
                    let mut indices = slice_selection.indices.clone().unwrap_or_default();
                    let mut slider = slice_selection.slider.clone().unwrap_or_default();
                    indices.push(new_selection.into());
                    slider.push(new_selection.dimension.into()); // Enable slider by default.
                    slice_property.save_blueprint_component(ctx, &indices);
                    slice_property.save_blueprint_component(ctx, &slider);
                }
            }
        };
    }
}

fn drag_source_ui_id(drag_context_id: egui::Id, dim_idx: u32) -> egui::Id {
    drag_context_id.with("tensor_dimension_ui").with(dim_idx)
}

#[allow(clippy::too_many_arguments)]
fn tensor_dimension_ui(
    ui: &mut egui::Ui,
    drag_context_id: egui::Id,
    bound_dim_idx: Option<u32>,
    location: DragDropAddress,
    shape: &[TensorDimension],
    drag_source: &mut DragDropAddress,
    drop_target: &mut DragDropAddress,
) {
    let frame = egui::Frame::default().inner_margin(4.0);

    let (_response, dropped) = ui.dnd_drop_zone::<DragDropAddress, _>(frame, |ui| {
        ui.set_min_size(egui::vec2(80., 15.));

        if let Some(dim_idx) = bound_dim_idx {
            let dim = &shape[dim_idx as usize];
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
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    shape: &[TensorDimension],
    slice_selection: &TensorSliceSelection,
    slice_property: &ViewProperty,
) {
    let mut drag_source = DragDropAddress::None; // Drag this…
    let mut drop_target = DragDropAddress::None; // …onto this.

    let drag_context_id = ui.id();

    ui.vertical(|ui| {
        ui.vertical(|ui| {
            ui.label("Image");
            egui::Grid::new("imagegrid").num_columns(2).show(ui, |ui| {
                tensor_dimension_ui(
                    ui,
                    drag_context_id,
                    slice_selection.width.map(|w| w.dimension),
                    DragDropAddress::Width,
                    shape,
                    &mut drag_source,
                    &mut drop_target,
                );
                ui.horizontal(|ui| {
                    if let Some(mut width) = slice_selection.width {
                        if ui.toggle_value(&mut width.invert, "Flip").changed() {
                            slice_property.save_blueprint_component(ctx, &width);
                        }
                    }
                    ui.label("width");
                });
                ui.end_row();

                tensor_dimension_ui(
                    ui,
                    drag_context_id,
                    slice_selection.height.map(|h| h.dimension),
                    DragDropAddress::Height,
                    shape,
                    &mut drag_source,
                    &mut drop_target,
                );

                ui.horizontal(|ui| {
                    if let Some(mut height) = slice_selection.height {
                        if ui.toggle_value(&mut height.invert, "Flip").changed() {
                            slice_property.save_blueprint_component(ctx, &height);
                        }
                    }
                    ui.label("height");
                });
                ui.end_row();
            });
        });

        ui.add_space(4.0);

        ui.vertical(|ui| {
            ui.label("Selectors");

            let Some(indices) = &slice_selection.indices else {
                return;
            };

            // Use Grid instead of Vertical layout to match styling of the parallel Grid for
            egui::Grid::new("selectiongrid")
                .num_columns(2)
                .show(ui, |ui| {
                    for (selector_idx, selector) in indices.iter().enumerate() {
                        tensor_dimension_ui(
                            ui,
                            drag_context_id,
                            Some(selector.dimension),
                            DragDropAddress::Selector(selector_idx),
                            shape,
                            &mut drag_source,
                            &mut drop_target,
                        );

                        let mut has_slider =
                            slice_selection.slider.as_ref().map_or(false, |slider| {
                                slider
                                    .iter()
                                    .any(|slider| slider.dimension == selector.dimension)
                            });

                        let response = ui.visibility_toggle_button(&mut has_slider);
                        let response = if has_slider {
                            response.on_hover_text("Hide dimension slider")
                        } else {
                            response.on_hover_text("Show dimension slider")
                        };
                        if response.changed() {
                            let mut slider = slice_selection.slider.clone().unwrap_or_default();
                            if has_slider {
                                slider.push(selector.dimension.into());
                            } else {
                                slider.retain(|slider| slider.dimension != selector.dimension);
                            }
                            slice_property.save_blueprint_component(ctx, &slider);
                        }

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
        let previous_value_source = drag_source.read_from_address(slice_selection, shape);
        let previous_value_target = drop_target.read_from_address(slice_selection, shape);
        drag_source.write_to_address(ctx, slice_selection, slice_property, previous_value_target);
        drop_target.write_to_address(ctx, slice_selection, slice_property, previous_value_source);
    }
}
