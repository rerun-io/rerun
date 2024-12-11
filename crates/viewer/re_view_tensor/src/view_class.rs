use egui::{epaint::TextShape, Align2, NumExt as _, Vec2};
use ndarray::Axis;

use re_data_ui::tensor_summary_ui_grid_contents;
use re_log_types::EntityPath;
use re_types::{
    blueprint::{
        archetypes::{TensorScalarMapping, TensorSliceSelection, TensorViewFit},
        components::ViewFit,
    },
    components::{Colormap, GammaCorrection, MagnificationFilter, TensorDimensionIndexSelection},
    datatypes::TensorData,
    View, ViewClassIdentifier,
};
use re_ui::{list_item, UiExt as _};
use re_view::{suggest_view_for_each_entity, view_property_ui};
use re_viewer_context::{
    gpu_bridge, ApplicableEntities, ColormapWithRange, IdentifiedViewSystem as _,
    IndicatedEntities, PerVisualizer, TensorStatsCache, TypedComponentFallbackProvider, ViewClass,
    ViewClassRegistryError, ViewId, ViewQuery, ViewState, ViewStateExt as _,
    ViewSystemExecutionError, ViewerContext, VisualizableEntities,
};
use re_viewport_blueprint::ViewProperty;

use crate::{
    dimension_mapping::load_tensor_slice_selection_and_make_valid,
    tensor_dimension_mapper::dimension_mapping_ui,
    visualizer_system::{TensorSystem, TensorVisualization},
    TensorDimension,
};

#[derive(Default)]
pub struct TensorView;

type ViewType = re_types::blueprint::views::TensorView;

#[derive(Default)]
pub struct ViewTensorState {
    /// Last viewed tensor, copied each frame.
    /// Used for the selection view.
    tensor: Option<TensorVisualization>,
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

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "# Tensor view

Display an N-dimensional tensor as an arbitrary 2D slice with custom colormap.

Note: select the view to configure which dimensions are shown."
            .to_owned()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
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
        _applicable_entities_per_visualizer: &PerVisualizer<ApplicableEntities>,
        visualizable_entities_per_visualizer: &PerVisualizer<VisualizableEntities>,
        _indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> re_viewer_context::SmallVisualizerSet {
        // Default implementation would not suggest the Tensor visualizer for images,
        // since they're not indicated with a Tensor indicator.
        // (and as of writing, something needs to be both visualizable and indicated to be shown in a visualizer)

        // Keeping this implementation simple: We know there's only a single visualizer here.
        if visualizable_entities_per_visualizer
            .get(&TensorSystem::identifier())
            .map_or(false, |entities| entities.contains(entity_path))
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
        _space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<ViewTensorState>()?;

        // TODO(andreas): Listitemify
        ui.selection_grid("tensor_selection_ui").show(ui, |ui| {
            if let Some(TensorVisualization {
                tensor,
                tensor_row_id,
                ..
            }) = &state.tensor
            {
                let tensor_stats = ctx
                    .cache
                    .entry(|c: &mut TensorStatsCache| c.entry(*tensor_row_id, tensor));

                tensor_summary_ui_grid_contents(ui, tensor, &tensor_stats);
            }
        });

        list_item::list_item_scope(ui, "tensor_selection_ui", |ui| {
            view_property_ui::<TensorScalarMapping>(ctx, ui, view_id, self, state);
            view_property_ui::<TensorViewFit>(ctx, ui, view_id, self, state);
        });

        // TODO(#6075): Listitemify
        if let Some(TensorVisualization { tensor, .. }) = &state.tensor {
            let slice_property = ViewProperty::from_archetype::<TensorSliceSelection>(
                ctx.blueprint_db(),
                ctx.blueprint_query,
                view_id,
            );
            let slice_selection = load_tensor_slice_selection_and_make_valid(
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
                    egui::Button::new("Reset to default"),
                )
                .on_hover_text("Reset dimension mapping to the default, i.e. as if never set")
                .on_disabled_hover_text("No custom dimension mapping set")
                .clicked()
            {
                slice_property.reset_all_components_to_empty(ctx);
            }
        }

        Ok(())
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        // For tensors create one view for each tensor (even though we're able to stack them in one view)
        suggest_view_for_each_entity::<TensorSystem>(ctx, self)
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
        state.tensor = None;

        let tensors = &system_output.view_systems.get::<TensorSystem>()?.tensors;

        if tensors.len() > 1 {
            egui::Frame {
                inner_margin: re_ui::DesignTokens::view_padding().into(),
                ..egui::Frame::default()
            }
            .show(ui, |ui| {
                ui.error_label(format!(
                    "Can only show one tensor at a time; was given {}. Update the query so that it \
                    returns a single tensor entity and create additional views for the others.",
                    tensors.len()
                ));
            });
        } else if let Some(tensor_view) = tensors.first() {
            state.tensor = Some(tensor_view.clone());
            self.view_tensor(ctx, ui, state, query.view_id, &tensor_view.tensor)?;
        } else {
            ui.centered_and_justified(|ui| ui.label("(empty)"));
        }

        Ok(())
    }
}

impl TensorView {
    fn view_tensor(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &ViewTensorState,
        view_id: ViewId,
        tensor: &TensorData,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let slice_property = ViewProperty::from_archetype::<TensorSliceSelection>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        );
        let slice_selection = load_tensor_slice_selection_and_make_valid(
            &slice_property,
            &TensorDimension::from_tensor_data(tensor),
        )?;

        let default_item_spacing = ui.spacing_mut().item_spacing;
        ui.spacing_mut().item_spacing.y = 0.0; // No extra spacing between sliders and tensor

        if slice_selection
            .slider
            .as_ref()
            .map_or(true, |s| !s.is_empty())
        {
            egui::Frame {
                inner_margin: egui::Margin::symmetric(16.0, 8.0),
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

        egui::ScrollArea::both().show(ui, |ui| {
            if let Err(err) =
                self.tensor_slice_ui(ctx, ui, state, view_id, dimension_labels, &slice_selection)
            {
                ui.error_label(err.to_string());
            }
        });

        Ok(())
    }

    fn tensor_slice_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &ViewTensorState,
        view_id: ViewId,
        dimension_labels: [Option<(String, bool)>; 2],
        slice_selection: &TensorSliceSelection,
    ) -> anyhow::Result<()> {
        let (response, painter, image_rect) =
            self.paint_tensor_slice(ctx, ui, state, view_id, slice_selection)?;

        if !response.hovered() {
            let font_id = egui::TextStyle::Body.resolve(ui.style());
            paint_axis_names(ui, &painter, image_rect, font_id, dimension_labels);
        }

        Ok(())
    }

    fn paint_tensor_slice(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &ViewTensorState,
        view_id: ViewId,
        slice_selection: &TensorSliceSelection,
    ) -> anyhow::Result<(egui::Response, egui::Painter, egui::Rect)> {
        re_tracing::profile_function!();

        let Some(tensor_view) = state.tensor.as_ref() else {
            anyhow::bail!("No tensor data available.");
        };
        let TensorVisualization {
            tensor_row_id,
            tensor,
            data_range,
        } = &tensor_view;

        let scalar_mapping = ViewProperty::from_archetype::<TensorScalarMapping>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        );
        let colormap: Colormap = scalar_mapping.component_or_fallback(ctx, self, state)?;
        let gamma: GammaCorrection = scalar_mapping.component_or_fallback(ctx, self, state)?;
        let mag_filter: MagnificationFilter =
            scalar_mapping.component_or_fallback(ctx, self, state)?;

        let Some(render_ctx) = ctx.render_ctx else {
            return Err(anyhow::Error::msg("No render context available."));
        };
        let colormap = ColormapWithRange {
            colormap,
            value_range: [data_range.start() as f32, data_range.end() as f32],
        };
        let colormapped_texture = super::tensor_slice_to_gpu::colormapped_texture(
            render_ctx,
            *tensor_row_id,
            tensor,
            slice_selection,
            &colormap,
            gamma,
        )?;
        let [width, height] = colormapped_texture.width_height();

        let view_fit: ViewFit = ViewProperty::from_archetype::<TensorViewFit>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        )
        .component_or_fallback(ctx, self, state)?;

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

        let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
        let rect = response.rect;
        let image_rect = egui::Rect::from_min_max(rect.min, rect.max);
        let texture_options = egui::TextureOptions {
            magnification: match mag_filter {
                MagnificationFilter::Nearest => egui::TextureFilter::Nearest,
                MagnificationFilter::Linear => egui::TextureFilter::Linear,
            },
            minification: egui::TextureFilter::Linear, // TODO(andreas): allow for mipmapping based filter
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: None,
        };

        gpu_bridge::render_image(
            render_ctx,
            &painter,
            image_rect,
            colormapped_texture,
            texture_options,
            re_renderer::DebugLabel::from("tensor_slice"),
        )?;

        Ok((response, painter, image_rect))
    }
}

// ----------------------------------------------------------------------------

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

    let empty_indices = Vec::new();
    let indices = indices.as_ref().unwrap_or(&empty_indices);

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
            .unwrap()
    } else {
        tensor.view()
    };

    #[allow(clippy::tuple_array_conversions)]
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

fn dimension_name(shape: &[TensorDimension], dim_idx: u32) -> String {
    let dim = &shape[dim_idx as usize];
    dim.name.as_ref().map_or_else(
        || format!("Dimension {dim_idx} (size={})", dim.size),
        |name| format!("{name} (size={})", dim.size),
    )
}

fn paint_axis_names(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    font_id: egui::FontId,
    dimension_labels: [Option<(String, bool)>; 2],
) {
    let [width, height] = dimension_labels;
    let (width_name, invert_width) =
        width.map_or((None, false), |(label, invert)| (Some(label), invert));
    let (height_name, invert_height) =
        height.map_or((None, false), |(label, invert)| (Some(label), invert));

    let text_color = ui.visuals().text_color();

    let rounding = re_ui::DesignTokens::normal_rounding();
    let inner_margin = rounding;
    let outer_margin = 8.0;

    let rect = rect.shrink(outer_margin + inner_margin);

    // We make sure that the label for the X axis is always at Y=0,
    // and that the label for the Y axis is always at X=0, no matter what inversions.
    //
    // For instance, with origin in the top right:
    //
    // foo ⬅
    // ..........
    // ..........
    // ..........
    // .......... ↓
    // .......... b
    // .......... a
    // .......... r

    // TODO(emilk): draw actual arrows behind the text instead of the ugly emoji arrows

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
    let mut indices = slice_selection.indices.clone().unwrap_or_default();

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
        slice_property.save_blueprint_component(ctx, &indices);
    }
}

impl TypedComponentFallbackProvider<Colormap> for TensorView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Colormap {
        // Viridis is a better fallback than Turbo for arbitrary tensors.
        Colormap::Viridis
    }
}

// Fallback for the various components of `TensorSliceSelection` is handled by `load_tensor_slice_selection_and_make_valid`.
re_viewer_context::impl_component_fallback_provider!(TensorView => [Colormap]);
