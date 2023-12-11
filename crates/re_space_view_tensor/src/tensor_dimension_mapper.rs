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
    can_accept_dragged: bool,
    bound_dim_idx: Option<usize>,
    location: DragDropAddress,
    shape: &[TensorDimension],
    drop_source: &mut DragDropAddress,
    drop_target: &mut DragDropAddress,
) {
    let response = drop_target_ui(ui, can_accept_dragged, |ui| {
        ui.set_min_size(egui::vec2(80., 15.));

        if let Some(dim_idx) = bound_dim_idx {
            let dim = &shape[dim_idx];
            let dim_ui_id = drag_source_ui_id(drag_context_id, dim_idx);

            let label_text = if let Some(dim_name) = dim.name.as_ref() {
                format!("▓ {dim_name} ({})", dim.size)
            } else {
                format!("▓ {dim_idx} ({})", dim.size)
            };

            drag_source_ui(ui, dim_ui_id, |ui| {
                // TODO(emilk): make these buttons respond on hover.
                ui.colored_label(ui.visuals().widgets.inactive.fg_stroke.color, label_text);
            });

            if ui.memory(|mem| mem.is_being_dragged(dim_ui_id)) {
                *drop_source = location;
            }
        }
    })
    .response;

    let is_being_dragged = ui.memory(|mem| mem.is_anything_being_dragged());
    if is_being_dragged && response.hovered() {
        *drop_target = location;
    }
}

fn drag_source_ui(ui: &mut egui::Ui, id: egui::Id, body: impl FnOnce(&mut egui::Ui)) {
    let is_being_dragged = ui.memory(|mem| mem.is_being_dragged(id));

    if !is_being_dragged {
        let response = ui.scope(body).response;

        // Check for drags:
        let response = ui.interact(response.rect, id, egui::Sense::drag());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    } else {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        // Paint the body to a new layer:
        let layer_id = egui::LayerId::new(egui::Order::Tooltip, id);
        let response = ui.with_layer_id(layer_id, body).response;

        // Now we move the visuals of the body to where the mouse is.
        // Normally you need to decide a location for a widget first,
        // because otherwise that widget cannot interact with the mouse.
        // However, a dragged component cannot be interacted with anyway
        // (anything with `Order::Tooltip` always gets an empty [`Response`])
        // So this is fine!

        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let delta = pointer_pos - response.rect.center();
            ui.ctx().translate_layer(layer_id, delta);
        }
    }
}

// Draws rectangle for a drop landing zone for dimensions
fn drop_target_ui<R>(
    ui: &mut egui::Ui,
    can_accept_dragged: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let is_being_dragged = ui.memory(|mem| mem.is_anything_being_dragged());

    let margin = egui::Vec2::splat(4.0);

    let outer_rect_bounds = ui.available_rect_before_wrap();
    let inner_rect = outer_rect_bounds.shrink2(margin);
    let where_to_put_background = ui.painter().add(egui::Shape::Noop);
    let mut content_ui = ui.child_ui(inner_rect, *ui.layout());
    let ret = body(&mut content_ui);
    let outer_rect =
        egui::Rect::from_min_max(outer_rect_bounds.min, content_ui.min_rect().max + margin);
    let (rect, response) = ui.allocate_at_least(outer_rect.size(), egui::Sense::hover());

    let style = if is_being_dragged && can_accept_dragged && response.hovered() {
        ui.visuals().widgets.active
    } else {
        ui.visuals().widgets.inactive
    };

    let mut fill = style.bg_fill;
    let mut stroke = style.bg_stroke;
    if is_being_dragged && !can_accept_dragged {
        fill = ui.visuals().gray_out(fill);
        stroke.color = ui.visuals().gray_out(stroke.color);
    }

    ui.painter().set(
        where_to_put_background,
        egui::epaint::RectShape::new(rect, style.rounding, fill, stroke),
    );

    egui::InnerResponse::new(ret, response)
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

    let mut drop_source = DragDropAddress::None;
    let mut drop_target = DragDropAddress::None;

    let drag_context_id = ui.id();
    let can_accept_dragged = (0..shape.len()).any(|dim_idx| {
        ui.memory(|mem| mem.is_being_dragged(drag_source_ui_id(drag_context_id, dim_idx)))
    });

    ui.vertical(|ui| {
        ui.vertical(|ui| {
            ui.strong("Image");
            egui::Grid::new("imagegrid").num_columns(2).show(ui, |ui| {
                tensor_dimension_ui(
                    ui,
                    drag_context_id,
                    can_accept_dragged,
                    dim_mapping.width,
                    DragDropAddress::Width,
                    shape,
                    &mut drop_source,
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
                    can_accept_dragged,
                    dim_mapping.height,
                    DragDropAddress::Height,
                    shape,
                    &mut drop_source,
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
                            can_accept_dragged,
                            Some(selector.dim_idx),
                            DragDropAddress::Selector(selector_idx),
                            shape,
                            &mut drop_source,
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
                            can_accept_dragged,
                            None,
                            DragDropAddress::NewSelector,
                            shape,
                            &mut drop_source,
                            &mut drop_target,
                        );
                        ui.end_row();
                    }
                });
        });
    });

    // persist drag/drop
    if drop_target.is_some() && drop_source.is_some() && ui.input(|i| i.pointer.any_released()) {
        let previous_value_source = drop_source.read_from_address(dim_mapping);
        let previous_value_target = drop_target.read_from_address(dim_mapping);
        drop_source.write_to_address(dim_mapping, previous_value_target);
        drop_target.write_to_address(dim_mapping, previous_value_source);
    }
}
