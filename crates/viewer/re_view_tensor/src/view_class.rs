use egui::epaint::TextShape;
use egui::{Align2, NumExt as _, Vec2};
use ndarray::Axis;
use re_data_ui::tensor_summary_ui_grid_contents;
use re_log_types::EntityPath;
use re_log_types::hash::Hash64;
use re_renderer::{
    renderer::{RectangleOptions, TexturedRect},
    ViewBuilder,
};
use re_sdk_types::blueprint::archetypes::{self, TensorScalarMapping, TensorViewFit};
use re_sdk_types::blueprint::components::ViewFit;
use re_sdk_types::components::{
    Colormap, GammaCorrection, MagnificationFilter, TensorData, TensorDimensionIndexSelection,
};
use re_sdk_types::tensor_data::TensorDataType;
use macaw;
use re_sdk_types::external::glam;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, UiExt as _, list_item};
use re_view::view_property_ui;
use re_viewer_context::{
    ColormapWithRange, IdentifiedViewSystem as _, IndicatedEntities, Item, PerVisualizer,
    PerVisualizerInViewClass, SystemCommand, SystemCommandSender as _, TensorStatsCache, ViewClass,
    ViewClassExt as _, ViewClassRegistryError, ViewContext, ViewId, ViewQuery, ViewState,
    ViewStateExt as _, ViewSystemExecutionError, ViewerContext, VisualizableEntities, gpu_bridge,
    suggest_view_for_each_entity,
};
use re_viewport_blueprint::ViewProperty;

use crate::TensorDimension;
use crate::dimension_mapping::TensorSliceSelection;
use crate::tensor_dimension_mapper::dimension_mapping_ui;
use crate::visualizer_system::{TensorSystem, TensorVisualization};

// --- Helper functions for TensorView ---

pub fn selected_tensor_slice<'a, T: Copy>(
    slice_selection: &TensorSliceSelection,
    tensor: &'a ndarray::ArrayViewD<'_, T>,
) -> ndarray::ArrayViewD<'a, T> {
    let TensorSliceSelection {
        width,
        height,
        indices,
        slider: _,
    } = slice_selection;

    let (dwidth, dheight) = if let (Some(width), Some(height)) = (width, height) {
        (width.dimension, height.dimension)
    } else if let Some(width) = width {
        // If height is missing, create a 1D row.
        (width.dimension, 1)
    } else if let Some(height) = height {
        // If width is missing, create a 1D column.
        (1, height.dimension)
    } else {
        // If both are missing, give up.
        return tensor.view();
    };

    let view = if tensor.shape().len() == 1 {
        // We want 2D slices, so for "pure" 1D tensors add a dimension.
        // This is important for above width/height conversion to work since this assumes at least 2 dimensions.
        tensor
            .view()
            .into_shape_with_order(ndarray::IxDyn(&[tensor.len(), 1]))
            .expect("Tensor.shape.len() is not actually 1!")
    } else {
        tensor.view()
    };

    let axis = [dheight as usize, dwidth as usize]
        .into_iter()
        .chain(indices.iter().map(|s| s.dimension as usize))
        .collect::<Vec<_>>();
    let mut slice = view.permuted_axes(axis);

    for index_selection in indices {
        // 0 and 1 are width/height, the rest are rearranged by dimension_mapping.selectors
        // This call removes Axis(2), so the next iteration of the loop does the right thing again.
        slice.index_axis_inplace(Axis(2), index_selection.index as usize);
    }
    if height.unwrap_or_default().invert {
        slice.invert_axis(Axis(0));
    }
    if width.unwrap_or_default().invert {
        slice.invert_axis(Axis(1));
    }

    slice
}

pub fn tensor_slice_shape(
    tensor: &TensorData,
    slice_selection: &TensorSliceSelection,
) -> Option<(usize, usize)> {
    macro_rules! get_shape {
        ($T:ty) => {{
            let view = ndarray::ArrayViewD::<$T>::try_from(&tensor.0).ok()?;
            let slice = selected_tensor_slice(slice_selection, &view);
            let shape = slice.shape();
            if shape.len() >= 2 {
                Some((shape[0], shape[1]))
            } else {
                None
            }
        }};
    }

    match tensor.dtype() {
        TensorDataType::U8 => get_shape!(u8),
        TensorDataType::U16 => get_shape!(u16),
        TensorDataType::U32 => get_shape!(u32),
        TensorDataType::U64 => get_shape!(u64),
        TensorDataType::I8 => get_shape!(i8),
        TensorDataType::I16 => get_shape!(i16),
        TensorDataType::I32 => get_shape!(i32),
        TensorDataType::I64 => get_shape!(i64),
        TensorDataType::F16 => get_shape!(half::f16),
        TensorDataType::F32 => get_shape!(f32),
        TensorDataType::F64 => get_shape!(f64),
    }
}

fn dimension_name(shape: &[TensorDimension], dim_idx: u32) -> String {
    let dim = &shape[dim_idx as usize];
    dim.name.as_ref().map_or_else(
        || format!("Dimension {dim_idx} (size={})", dim.size),
        |name| format!("{name} (size={})", dim.size),
    )
}

fn paint_axis_names(
    ui: &egui::Ui,
    rect: egui::Rect,
    font_id: egui::FontId,
    dimension_labels: [Option<(String, bool)>; 2],
) {
    let painter = ui.painter();
    let tokens = ui.tokens();

    let [width, height] = dimension_labels;
    let (width_name, invert_width) =
        width.map_or((None, false), |(label, invert)| (Some(label), invert));
    let (height_name, invert_height) =
        height.map_or((None, false), |(label, invert)| (Some(label), invert));

    let text_color = ui.visuals().text_color();

    let rounding = tokens.normal_corner_radius();
    let inner_margin = rounding as f32;
    let outer_margin = 8.0;

    let rect = rect.shrink(outer_margin + inner_margin);

    let paint_text_bg = |text_background, text_rect: egui::Rect| {
        painter.set(
            text_background,
            egui::Shape::rect_filled(
                text_rect.expand(inner_margin),
                rounding,
                ui.visuals().panel_fill,
            ),
        );
    };

    // Label for X axis:
    if let Some(width_name) = width_name {
        let text_background = painter.add(egui::Shape::Noop);
        let text_rect = if invert_width {
            // On left, pointing left:
            let (pos, align) = if invert_height {
                (rect.left_bottom(), Align2::LEFT_BOTTOM)
            } else {
                (rect.left_top(), Align2::LEFT_TOP)
            };
            painter.text(
                pos,
                align,
                format!("{width_name} ⬅"),
                font_id.clone(),
                text_color,
            )
        } else {
            // On right, pointing right:
            let (pos, align) = if invert_height {
                (rect.right_bottom(), Align2::RIGHT_BOTTOM)
            } else {
                (rect.right_top(), Align2::RIGHT_TOP)
            };
            painter.text(
                pos,
                align,
                format!("➡ {width_name}"),
                font_id.clone(),
                text_color,
            )
        };
        paint_text_bg(text_background, text_rect);
    }

    // Label for Y axis:
    if let Some(height_name) = height_name {
        let text_background = painter.add(egui::Shape::Noop);
        let text_rect = if invert_height {
            // On top, pointing up:
            let galley = painter.layout_no_wrap(format!("➡ {height_name}"), font_id, text_color);
            let galley_size = galley.size();
            let pos = if invert_width {
                rect.right_top() + egui::vec2(-galley_size.y, galley_size.x)
            } else {
                rect.left_top() + egui::vec2(0.0, galley_size.x)
            };
            painter.add(
                TextShape::new(pos, galley, text_color).with_angle(-std::f32::consts::TAU / 4.0),
            );
            egui::Rect::from_min_size(
                pos - galley_size.x * egui::Vec2::Y,
                egui::vec2(galley_size.y, galley_size.x),
            )
        } else {
            // On bottom, pointing down:
            let galley = painter.layout_no_wrap(format!("{height_name} ⬅"), font_id, text_color);
            let galley_size = galley.size();
            let pos = if invert_width {
                rect.right_bottom() - egui::vec2(galley_size.y, 0.0)
            } else {
                rect.left_bottom()
            };
            painter.add(
                TextShape::new(pos, galley, text_color).with_angle(-std::f32::consts::TAU / 4.0),
            );
            egui::Rect::from_min_size(
                pos - galley_size.x * egui::Vec2::Y,
                egui::vec2(galley_size.y, galley_size.x),
            )
        };
        paint_text_bg(text_background, text_rect);
    }
}

pub fn index_for_dimension_mut(
    indices: &mut [TensorDimensionIndexSelection],
    dimension: u32,
) -> Option<&mut u64> {
    indices
        .iter_mut()
        .find(|index| index.dimension == dimension)
        .map(|index| &mut index.index)
}

fn selectors_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    shape: &[TensorDimension],
    slice_selection: &TensorSliceSelection,
    slice_property: &ViewProperty,
) {
    let Some(slider) = &slice_selection.slider else {
        return;
    };

    let mut changed_indices = false;
    let mut indices = slice_selection.indices.clone();

    for index_slider in slider {
        let dim = &shape[index_slider.dimension as usize];
        let size = dim.size;
        if size <= 1 {
            continue;
        }

        let Some(selector_value) = index_for_dimension_mut(&mut indices, index_slider.dimension)
        else {
            // There should be an entry already via `load_tensor_slice_selection_and_make_valid`
            continue;
        };

        ui.horizontal(|ui| {
            let name = dim.name.clone().map_or_else(
                || index_slider.dimension.to_string(),
                |name| name.to_string(),
            );

            let slider_tooltip = format!("Adjust the selected slice for the {name} dimension");
            ui.label(&name).on_hover_text(&slider_tooltip);

            // If the range is big (say, 2048) then we would need
            // a slider that is 2048 pixels wide to get the good precision.
            // So we add a high-precision drag-value instead:
            if ui
                .add(
                    egui::DragValue::new(selector_value)
                        .range(0..=size - 1)
                        .speed(0.5),
                )
                .on_hover_text(format!(
                    "Drag to precisely control the slice index of the {name} dimension"
                ))
                .changed()
            {
                changed_indices = true;
            }

            // Make the slider as big as needed:
            const MIN_SLIDER_WIDTH: f32 = 64.0;
            if ui.available_width() >= MIN_SLIDER_WIDTH {
                ui.spacing_mut().slider_width = ((size as f32) * 4.0)
                    .at_least(MIN_SLIDER_WIDTH)
                    .at_most(ui.available_width());
                if ui
                    .add(egui::Slider::new(selector_value, 0..=size - 1).show_value(false))
                    .on_hover_text(slider_tooltip)
                    .changed()
                {
                    changed_indices = true;
                }
            }
        });
    }

    if changed_indices {
        slice_property.save_blueprint_component(
            ctx,
            &archetypes::TensorSliceSelection::descriptor_indices(),
            &indices,
        );
    }
}

// --- Main TensorView impl ---

#[derive(Default)]
pub struct TensorView;

type ViewType = re_sdk_types::blueprint::views::TensorView;

pub struct ViewTensorState {
    /// Last viewed tensors, copied each frame.
    /// Used for the selection view.
    tensors: Vec<TensorVisualization>,
    pan: egui::Vec2,
    zoom: f32,
}

impl Default for ViewTensorState {
    fn default() -> Self {
        Self {
            tensors: Vec::new(),
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl ViewState for ViewTensorState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ViewClass for TensorView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Tensor"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_TENSOR
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Tensor view")
            .docs_link("https://rerun.io/docs/reference/types/views/tensor_view")
            .markdown(
                "An N-dimensional tensor displayed as a 2D slice with a custom colormap.

Set the displayed dimensions in a selection panel.",
            )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_fallback_provider(
            TensorScalarMapping::descriptor_colormap().component,
            |_| Colormap::Viridis,
        );

        system_registry.register_visualizer::<TensorSystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Medium
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<ViewTensorState>::default()
    }

    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizerInViewClass<VisualizableEntities>,
        _indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> re_viewer_context::SmallVisualizerSet {
        // Default implementation would not suggest the Tensor visualizer for images,
        // since they're not indicated with a Tensor indicator.
        // (and as of writing, something needs to be both visualizable and indicated to be shown in a visualizer)

        // Keeping this implementation simple: We know there's only a single visualizer here.
        if visualizable_entities_per_visualizer
            .get(&TensorSystem::identifier())
            .is_some_and(|entities| entities.contains_key(entity_path))
        {
            std::iter::once(TensorSystem::identifier()).collect()
        } else {
            Default::default()
        }
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<ViewTensorState>()?;

        // TODO(andreas): Listitemify
        ui.selection_grid("tensor_selection_ui").show(ui, |ui| {
            for TensorVisualization {
                tensor,
                tensor_row_id,
                ..
            } in &state.tensors
            {
                let tensor_stats = ctx.store_context.caches.entry(|c: &mut TensorStatsCache| {
                    c.entry(Hash64::hash(*tensor_row_id), tensor)
                });

                tensor_summary_ui_grid_contents(ui, tensor, &tensor_stats);
            }
        });

        list_item::list_item_scope(ui, "tensor_selection_ui", |ui| {
            let ctx = self.view_context(ctx, view_id, state, space_origin);
            view_property_ui::<TensorScalarMapping>(&ctx, ui);
            view_property_ui::<TensorViewFit>(&ctx, ui);
        });

        // TODO(#6075): Listitemify
        if let Some(TensorVisualization { tensor, .. }) = state.tensors.first() {
            let slice_property = ViewProperty::from_archetype::<
                re_sdk_types::blueprint::archetypes::TensorSliceSelection,
            >(ctx.blueprint_db(), ctx.blueprint_query, view_id);
            let slice_selection = TensorSliceSelection::load_and_make_valid(
                &slice_property,
                &TensorDimension::from_tensor_data(tensor),
            )?;

            ui.separator();
            ui.strong("Dimension Mapping");
            dimension_mapping_ui(
                ctx,
                ui,
                &TensorDimension::from_tensor_data(tensor),
                &slice_selection,
                &slice_property,
            );

            // TODO(andreas): this is a bit too inconsistent with the other UIs - we don't offer the same reset/option buttons here
            if ui
                .button("Reset to default blueprint")
                .on_hover_text("Reset dimension mapping to the previously set default blueprint")
                .clicked()
            {
                slice_property.reset_all_components(ctx);
            }

            if ui
                .add_enabled(
                    slice_property.any_non_empty(),
                    egui::Button::new("Reset to heuristic"),
                )
                .on_hover_text("Reset dimension mapping to the heuristic, i.e. as if never set")
                .on_disabled_hover_text("No custom dimension mapping set")
                .clicked()
            {
                slice_property.reset_all_components_to_empty(ctx);
            }
        }

        Ok(())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        // For tensors create one view for each tensor (even though we're able to stack them in one view)
        suggest_view_for_each_entity::<TensorSystem>(ctx, include_entity)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<ViewTensorState>()?;
        state.tensors.clear();

        let tensors = &system_output.view_systems.get::<TensorSystem>()?.tensors;

        let response = {
            let mut ui = ui.new_child(egui::UiBuilder::new().sense(egui::Sense::click()));

            if tensors.is_empty() {
                ui.centered_and_justified(|ui| ui.label("(empty)"));
            } else {
                state.tensors = tensors.clone();
                self.view_tensor(
                    ctx,
                    &mut ui,
                    state,
                    query.view_id,
                    query.space_origin,
                    tensors,
                )?;
            }

            ui.response()
        };

        if response.hovered() {
            ctx.selection_state().set_hovered(Item::View(query.view_id));
        }

        if response.clicked() {
            ctx.command_sender()
                .send_system(SystemCommand::set_selection(Item::View(query.view_id)));
        }

        Ok(())
    }
}

impl TensorView {
    fn view_tensor(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut ViewTensorState,
        view_id: ViewId,
        space_origin: &EntityPath,
        tensors: &[TensorVisualization],
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        // Use the first tensor for slice selection
        let tensor = &tensors[0].tensor;

        let slice_property = ViewProperty::from_archetype::<
            re_sdk_types::blueprint::archetypes::TensorSliceSelection,
        >(ctx.blueprint_db(), ctx.blueprint_query, view_id);
        let slice_selection = TensorSliceSelection::load_and_make_valid(
            &slice_property,
            &TensorDimension::from_tensor_data(tensor),
        )?;

        let default_item_spacing = ui.spacing_mut().item_spacing;
        ui.spacing_mut().item_spacing.y = 0.0; // No extra spacing between sliders and tensor

        if slice_selection
            .slider
            .as_ref()
            .is_none_or(|s| !s.is_empty())
        {
            egui::Frame {
                inner_margin: egui::Margin::symmetric(16, 8),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = default_item_spacing; // keep the default spacing between sliders
                selectors_ui(
                    ctx,
                    ui,
                    &TensorDimension::from_tensor_data(tensor),
                    &slice_selection,
                    &slice_property,
                );
            });
        }

        let dimension_labels = [
            slice_selection.width.map(|width| {
                (
                    dimension_name(&TensorDimension::from_tensor_data(tensor), width.dimension),
                    width.invert,
                )
            }),
            slice_selection.height.map(|height| {
                (
                    dimension_name(&TensorDimension::from_tensor_data(tensor), height.dimension),
                    height.invert,
                )
            }),
        ];

        let mut pan = state.pan;
        let mut zoom = state.zoom;

        egui::ScrollArea::both()
            .auto_shrink(false)
            .scroll_source(
                egui::scroll_area::ScrollSource::SCROLL_BAR
                    | egui::scroll_area::ScrollSource::DRAG,
            )
            .show(ui, |ui| {
                let ctx = self.view_context(ctx, view_id, state, space_origin);
                if let Err(err) = Self::tensor_slice_ui(
                    &ctx,
                    ui,
                    tensors,
                    dimension_labels,
                    &slice_selection,
                    &slice_property,
                    tensor,
                    &mut pan,
                    &mut zoom,
                ) {
                    ui.error_label(err.to_string());
                }
            });

        state.pan = pan;
        state.zoom = zoom;

        Ok(())
    }

    fn tensor_slice_ui(
        ctx: &ViewContext<'_>,
        ui: &mut egui::Ui,
        tensors: &[TensorVisualization],
        dimension_labels: [Option<(String, bool)>; 2],
        slice_selection: &TensorSliceSelection,
        slice_property: &ViewProperty,
        tensor: &TensorData,
        pan: &mut egui::Vec2,
        zoom: &mut f32,
    ) -> anyhow::Result<()> {
        let mag_filter = ViewProperty::from_archetype::<TensorScalarMapping>(
            ctx.blueprint_db(),
            ctx.blueprint_query(),
            ctx.view_id,
        )
        .component_or_fallback(ctx, TensorScalarMapping::descriptor_mag_filter().component)?;

        let (response, image_rect) =
            paint_tensor_slice(ctx, ui, tensors, slice_selection, mag_filter, *pan, *zoom)?;

        if response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta.abs() > 0.0 {
                if let Some(sliders) = &slice_selection.slider
                    && let Some(first_slider) = sliders.first()
                {
                    let mut indices = slice_selection.indices.clone();
                    if let Some(index) =
                        index_for_dimension_mut(&mut indices, first_slider.dimension)
                        && let Some(dim) =
                            TensorDimension::from_tensor_data(tensor).get(first_slider.dimension as usize)
                    {
                        let max_index = dim.size.saturating_sub(1) as i64;
                        let direction = if scroll_delta > 0.0 { 1_i64 } else { -1_i64 };
                        let current = *index as i64;
                        let new_index = (current + direction).clamp(0, max_index) as u64;
                        if *index != new_index {
                            *index = new_index;
                            slice_property.save_blueprint_component(
                                ctx.viewer_ctx,
                                &archetypes::TensorSliceSelection::descriptor_indices(),
                                &indices,
                            );
                        }
                    }
                }
            }
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some((height, width)) = tensor_slice_shape(tensor, slice_selection) {
                    let image_size = egui::vec2(width as f32, height as f32);
                    let space_size = image_size / *zoom;
                    let image_rect_size = image_rect.size();
                    if image_rect_size.x > 0.0 && image_rect_size.y > 0.0 {
                        let scale = egui::vec2(
                            space_size.x / image_rect_size.x,
                            space_size.y / image_rect_size.y,
                        );
                        *pan += response.drag_delta() * scale;
                    }
                }
            }
            if response.dragged_by(egui::PointerButton::Secondary) {
                let delta = response.drag_delta().y;
                if delta.abs() > 0.0 {
                    let factor = (1.0 + delta * 0.01).clamp(0.2, 5.0);
                    *zoom = (*zoom * factor).clamp(1.0, 20.0);
                }
            }
            if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                response.clone().on_hover_ui_at_pointer(|ui| {
                    crate::tensor_slice_hover::show_tensor_hover_ui(
                        ctx,
                        ui,
                        tensors,
                        slice_selection,
                        image_rect,
                        pointer_pos,
                        mag_filter,
                    );
                });
            }
        } else {
            let font_id = egui::TextStyle::Body.resolve(ui.style());
            paint_axis_names(ui, image_rect, font_id, dimension_labels);
        }

        if response.double_clicked() {
            *pan = egui::Vec2::ZERO;
            *zoom = 1.0;
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paint_tensor_slice(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    tensors: &[TensorVisualization],
    slice_selection: &TensorSliceSelection,
    mag_filter: MagnificationFilter,
    pan: egui::Vec2,
    zoom: f32,
) -> anyhow::Result<(egui::Response, egui::Rect)> {
    re_tracing::profile_function!();

    if tensors.is_empty() {
        anyhow::bail!("No tensor data available.");
    }

    let first_tensor_view = &tensors[0];
    let Some((height, width)) = tensor_slice_shape(&first_tensor_view.tensor, slice_selection)
    else {
        anyhow::bail!("Expected a 2D tensor slice.");
    };

    let view_fit: ViewFit = ViewProperty::from_archetype::<TensorViewFit>(
        ctx.blueprint_db(),
        ctx.blueprint_query(),
        ctx.view_id,
    )
    .component_or_fallback(ctx, TensorViewFit::descriptor_scaling().component)?;

    let img_size = egui::vec2(width as _, height as _);
    let img_size = Vec2::max(Vec2::splat(1.0), img_size); // better safe than sorry
    let desired_size = match view_fit {
        ViewFit::Original => img_size,
        ViewFit::Fill => ui.available_size(),
        ViewFit::FillKeepAspectRatio => {
            let scale = (ui.available_size() / img_size).min_elem();
            img_size * scale
        }
    };

    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::click_and_drag());
    let image_rect = egui::Rect::from_min_max(response.rect.min, response.rect.max);

    let zoom = zoom.max(1.0);
    let space_size = img_size / zoom;
    let image_center = img_size * 0.5;
    let mut space_center = egui::pos2(image_center.x + pan.x, image_center.y + pan.y);
    let half_size = space_size * 0.5;
    let max_center = img_size - half_size;
    space_center = egui::pos2(
        space_center.x.clamp(half_size.x, max_center.x),
        space_center.y.clamp(half_size.y, max_center.y),
    );
    let space_rect = egui::Rect::from_center_size(space_center, space_size);

    let (textured_rects, texture_filter_magnification) =
        create_textured_rects_for_batch(ctx, tensors, slice_selection, mag_filter)?;

    render_tensor_slice_batch(ctx, &painter, &textured_rects, image_rect, space_rect, texture_filter_magnification)?;

    Ok((response, image_rect))
}


#[allow(clippy::too_many_arguments)]
pub fn create_textured_rects_for_batch(
    ctx: &ViewContext<'_>,
    tensors: &[TensorVisualization],
    slice_selection: &TensorSliceSelection,
    mag_filter: MagnificationFilter,
) -> anyhow::Result<(Vec<TexturedRect>, re_renderer::renderer::TextureFilterMag)> {
    let first_tensor_view = &tensors[0];
    let TensorVisualization {
        tensor: first_tensor,
        ..
    } = first_tensor_view;

    let scalar_mapping = ViewProperty::from_archetype::<TensorScalarMapping>(
        ctx.blueprint_db(),
        ctx.blueprint_query(),
        ctx.view_id,
    );
    let colormap: Colormap =
        scalar_mapping.component_or_fallback(ctx, TensorScalarMapping::descriptor_colormap().component)?;
    let gamma: GammaCorrection =
        scalar_mapping.component_or_fallback(ctx, TensorScalarMapping::descriptor_gamma().component)?;

    let texture_filter_magnification = match mag_filter {
        MagnificationFilter::Nearest => re_renderer::renderer::TextureFilterMag::Nearest,
        MagnificationFilter::Linear => re_renderer::renderer::TextureFilterMag::Linear,
    };
    let texture_filter_minification = re_renderer::renderer::TextureFilterMin::Linear;

    let mut textured_rects = Vec::with_capacity(tensors.len());

    let Some((height, width)) = tensor_slice_shape(first_tensor, slice_selection) else {
        anyhow::bail!("Expected a 2D tensor slice.");
    };
    let space_rect = egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(width as f32, height as f32),
    );

    for tensor_view in tensors {
        let TensorVisualization {
            tensor_row_id,
            tensor,
            data_range,
            annotations,
            opacity,
            ..
        } = tensor_view;

        let colormap_with_range = ColormapWithRange {
            colormap,
            value_range: [data_range.start() as f32, data_range.end() as f32],
        };

        let colormapped_texture =
            crate::tensor_slice_to_gpu::colormapped_texture(
                ctx.render_ctx(),
                *tensor_row_id,
                tensor,
                slice_selection,
                annotations,
                &colormap_with_range,
                gamma,
            )?;

        let multiplicative_tint = egui::Rgba::from_white_alpha(*opacity);

        textured_rects.push(TexturedRect {
            top_left_corner_position: glam::vec3(space_rect.min.x, space_rect.min.y, 0.0),
            extent_u: glam::Vec3::X * space_rect.width(),
            extent_v: glam::Vec3::Y * space_rect.height(),
            colormapped_texture,
            options: RectangleOptions {
                texture_filter_magnification,
                texture_filter_minification,
                multiplicative_tint,
                ..Default::default()
            },
        });
    }

    Ok((textured_rects, texture_filter_magnification))
}

pub fn render_tensor_slice_batch(
    ctx: &ViewContext<'_>,
    painter: &egui::Painter,
    textured_rects: &[TexturedRect],
    image_rect: egui::Rect,
    space_rect: egui::Rect,
    _texture_filter_magnification: re_renderer::renderer::TextureFilterMag,
) -> anyhow::Result<()> {
    let viewport = painter.clip_rect().intersect(image_rect);
    if viewport.is_positive() {
        let pixels_per_point = painter.ctx().pixels_per_point();
        let resolution_in_pixel =
            gpu_bridge::viewport_resolution_in_pixels(viewport, pixels_per_point);

        if resolution_in_pixel[0] > 0 && resolution_in_pixel[1] > 0 {
            let ui_from_space = egui::emath::RectTransform::from_to(space_rect, image_rect);
            let space_from_ui = ui_from_space.inverse();
            
            let camera_position_space = space_from_ui.transform_pos(viewport.min);
            let top_left_position =
                glam::vec2(camera_position_space.x, camera_position_space.y);

            let target_config = re_renderer::view_builder::TargetConfiguration {
                name: "tensor_slice_batch".into(),
                resolution_in_pixel,
                view_from_world: macaw::IsoTransform::from_translation(
                    -top_left_position.extend(0.0),
                ),
                projection_from_view: re_renderer::view_builder::Projection::Orthographic {
                    camera_mode:
                        re_renderer::view_builder::OrthographicCameraMode::TopLeftCornerAndExtendZ,
                    vertical_world_size: space_rect.height(),
                    far_plane_distance: 1000.0,
                },
                viewport_transformation: re_renderer::RectTransform::IDENTITY,
                pixels_per_point,
                ..Default::default()
            };

            let mut view_builder = ViewBuilder::new(ctx.render_ctx(), target_config)?;

            view_builder.queue_draw(
                ctx.render_ctx(),
                re_renderer::renderer::RectangleDrawData::new(ctx.render_ctx(), textured_rects)?,
            );

            painter.add(gpu_bridge::new_renderer_callback(
                view_builder,
                viewport,
                re_renderer::Rgba::TRANSPARENT,
            ));
        }
    }

    Ok(())
}

// ----------------------------------------------------------------------------

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| TensorView.help(ctx));
}
