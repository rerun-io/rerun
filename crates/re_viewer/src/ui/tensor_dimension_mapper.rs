use re_log_types::TensorDimension;
use re_tensor_ops::dimension_mapping::DimensionMapping;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragDropAddress {
    None,
    Width,
    Height,
    Channel,
    Selector(usize),
    NewSelector,
}

fn get_drag_source_ui_id(ui_id: egui::Id, dim_idx: usize) -> egui::Id {
    ui_id.with("tensor_dimension_ui").with(dim_idx)
}

impl DragDropAddress {
    fn read_from_address(&self, dimension_mapping: &DimensionMapping) -> Option<usize> {
        match self {
            DragDropAddress::None => unreachable!(),
            DragDropAddress::Width => dimension_mapping.width,
            DragDropAddress::Height => dimension_mapping.height,
            DragDropAddress::Channel => dimension_mapping.channel,
            DragDropAddress::Selector(selector_idx) => {
                Some(dimension_mapping.selectors[*selector_idx])
            }
            DragDropAddress::NewSelector => None,
        }
    }

    fn write_to_address(&self, dim_idx: Option<usize>, dimension_mapping: &mut DimensionMapping) {
        match self {
            DragDropAddress::None => unreachable!(),
            DragDropAddress::Width => dimension_mapping.width = dim_idx,
            DragDropAddress::Height => dimension_mapping.height = dim_idx,
            DragDropAddress::Channel => dimension_mapping.channel = dim_idx,
            DragDropAddress::Selector(selector_idx) => {
                if let Some(dim_idx) = dim_idx {
                    dimension_mapping.selectors[*selector_idx] = dim_idx;
                } else {
                    dimension_mapping.selectors.remove(*selector_idx);
                }
            }
            // NewSelector can only be a drop *target*, therefore dim_idx can't be None!
            DragDropAddress::NewSelector => dimension_mapping.selectors.push(dim_idx.unwrap()),
        };
    }
}

fn tensor_dimension_ui(
    ui: &mut egui::Ui,
    can_accept_dragged: bool,
    bound_dim: Option<(usize, egui::Id)>,
    location: DragDropAddress,
    shape: &[TensorDimension],
    drop_source: &mut DragDropAddress,
    drop_target: &mut DragDropAddress,
) {
    let response = drop_target_ui(ui, can_accept_dragged, |ui| {
        ui.set_min_size(egui::vec2(80., 15.));

        if let Some((dim_idx, dim_ui_id)) = bound_dim {
            let dim = &shape[dim_idx];

            let tmp: String;
            let display_name = if dim.name.is_empty() {
                tmp = format!("{}", dim_idx);
                &tmp
            } else {
                &dim.name
            };

            drag_source_ui(ui, dim_ui_id, |ui| {
                ui.label(format!("▓ {} ({})", display_name, dim.size));
            });

            if ui.memory().is_being_dragged(dim_ui_id) {
                *drop_source = location;
            }
        }
    })
    .response;

    let is_being_dragged = ui.memory().is_anything_being_dragged();
    if is_being_dragged && response.hovered() {
        *drop_target = location;
    }
}

fn drag_source_ui(ui: &mut egui::Ui, id: egui::Id, body: impl FnOnce(&mut egui::Ui)) {
    let is_being_dragged = ui.memory().is_being_dragged(id);

    if !is_being_dragged {
        let response = ui.scope(body).response;

        // Check for drags:
        let response = ui.interact(response.rect, id, egui::Sense::drag());
        if response.hovered() {
            ui.output().cursor_icon = egui::CursorIcon::Grab;
        }
    } else {
        ui.output().cursor_icon = egui::CursorIcon::Grabbing;

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
    let is_being_dragged = ui.memory().is_anything_being_dragged();

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
        // gray out:
        fill = egui::color::tint_color_towards(fill, ui.visuals().window_fill());
        stroke.color = egui::color::tint_color_towards(stroke.color, ui.visuals().window_fill());
    }

    ui.painter().set(
        where_to_put_background,
        egui::epaint::RectShape {
            rounding: style.rounding,
            fill,
            stroke,
            rect,
        },
    );

    egui::InnerResponse::new(ret, response)
}

pub fn dimension_mapping_ui(
    ui: &mut egui::Ui,
    dimension_mapping: &mut DimensionMapping,
    shape: &[TensorDimension],
) {
    let mut drop_source = DragDropAddress::None;
    let mut drop_target = DragDropAddress::None;

    let drag_context_id = ui.id();
    let can_accept_dragged = (0..shape.len()).any(|dim_idx| {
        ui.memory()
            .is_being_dragged(get_drag_source_ui_id(drag_context_id, dim_idx))
    });

    ui.columns(2, |columns| {
        {
            let ui = &mut columns[0];
            ui.heading("Image:");
            egui::Grid::new("imagegrid").num_columns(2).show(ui, |ui| {
                ui.label("Width:");

                tensor_dimension_ui(
                    ui,
                    can_accept_dragged,
                    dimension_mapping
                        .width
                        .map(|dim_idx| (dim_idx, get_drag_source_ui_id(drag_context_id, dim_idx))),
                    DragDropAddress::Width,
                    shape,
                    &mut drop_source,
                    &mut drop_target,
                );
                ui.end_row();

                ui.label("Height:");
                tensor_dimension_ui(
                    ui,
                    can_accept_dragged,
                    dimension_mapping
                        .height
                        .map(|dim_idx| (dim_idx, get_drag_source_ui_id(drag_context_id, dim_idx))),
                    DragDropAddress::Height,
                    shape,
                    &mut drop_source,
                    &mut drop_target,
                );
                ui.end_row();

                ui.label("Channel:");
                tensor_dimension_ui(
                    ui,
                    can_accept_dragged,
                    dimension_mapping
                        .channel
                        .map(|dim_idx| (dim_idx, get_drag_source_ui_id(drag_context_id, dim_idx))),
                    DragDropAddress::Channel,
                    shape,
                    &mut drop_source,
                    &mut drop_target,
                );
                ui.end_row();
            });
        }
        {
            let ui = &mut columns[1];
            ui.heading("Selectors:");
            egui::Grid::new("selectiongrid")
                .num_columns(1)
                .show(ui, |ui| {
                    for (selector_idx, &mut dim_idx) in
                        dimension_mapping.selectors.iter_mut().enumerate()
                    {
                        tensor_dimension_ui(
                            ui,
                            can_accept_dragged,
                            Some((dim_idx, get_drag_source_ui_id(drag_context_id, dim_idx))),
                            DragDropAddress::Selector(selector_idx),
                            shape,
                            &mut drop_source,
                            &mut drop_target,
                        );
                        ui.end_row();
                    }
                    tensor_dimension_ui(
                        ui,
                        can_accept_dragged,
                        None,
                        DragDropAddress::NewSelector,
                        shape,
                        &mut drop_source,
                        &mut drop_target,
                    );
                    ui.end_row();
                });
        }
    });

    // persist drag/drop
    if drop_target != DragDropAddress::None
        && drop_source != DragDropAddress::None
        && ui.input().pointer.any_released()
    {
        let previous_value_source = drop_source.read_from_address(&dimension_mapping);
        let previous_value_target = drop_target.read_from_address(&dimension_mapping);
        drop_source.write_to_address(previous_value_target, dimension_mapping);
        drop_target.write_to_address(previous_value_source, dimension_mapping);
    }
}
