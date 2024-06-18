use std::collections::BTreeMap;

use egui::{epaint::TextShape, Align2, NumExt as _, Vec2};
use ndarray::Axis;
use re_space_view::{suggest_space_view_for_each_entity, view_property_ui};

use crate::dimension_mapping::{DimensionMapping, DimensionSelector};
use re_data_ui::tensor_summary_ui_grid_contents;
use re_log_types::{EntityPath, RowId};
use re_types::{
    blueprint::{
        archetypes::{TensorScalarMapping, TensorViewFit},
        components::ViewFit,
    },
    components::{Colormap, GammaCorrection, MagnificationFilter},
    datatypes::{TensorData, TensorDimension},
    tensor_data::{DecodedTensor, TensorDataMeaning},
    SpaceViewClassIdentifier, View,
};
use re_ui::{list_item, ContextExt as _, UiExt as _};
use re_viewer_context::{
    gpu_bridge, ApplicableEntities, IdentifiedViewSystem as _, IndicatedEntities, PerVisualizer,
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState,
    SpaceViewStateExt as _, SpaceViewSystemExecutionError, TensorStatsCache,
    TypedComponentFallbackProvider, ViewQuery, ViewerContext, VisualizableEntities,
};
use re_viewport_blueprint::ViewProperty;

use crate::{tensor_dimension_mapper::dimension_mapping_ui, visualizer_system::TensorSystem};

#[derive(Default)]
pub struct TensorSpaceView;

type ViewType = re_types::blueprint::views::TensorView;

#[derive(Default)]
pub struct ViewTensorState {
    /// What slice are we viewing?
    ///
    /// This get automatically reset if/when the current tensor shape changes.
    pub(crate) slice: SliceSelection,

    /// Last viewed tensor, copied each frame.
    /// Used for the selection view.
    tensor: Option<(RowId, DecodedTensor)>,
}

impl SpaceViewState for ViewTensorState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// How we slice a given tensor
#[derive(Default, Clone, Debug, Hash, PartialEq, Eq)]
pub struct SliceSelection {
    /// How we select which dimensions to project the tensor onto.
    pub dim_mapping: DimensionMapping,

    /// Selected value of every dimension (iff they are in [`DimensionMapping::selectors`]).
    pub selector_values: BTreeMap<usize, u64>,
}

impl SpaceViewClass for TensorSpaceView {
    fn identifier() -> SpaceViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Tensor"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TENSOR
    }

    fn help_text(&self, _egui_ctx: &egui::Context) -> egui::WidgetText {
        "Select the space view to configure which dimensions are shown.".into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<TensorSystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Medium
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
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
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<ViewTensorState>()?;

        // TODO(andreas): Listitemify
        ui.selection_grid("tensor_selection_ui").show(ui, |ui| {
            if let Some((tensor_data_row_id, tensor)) = &state.tensor {
                let tensor_stats = ctx
                    .cache
                    .entry(|c: &mut TensorStatsCache| c.entry(*tensor_data_row_id, tensor));

                // We are in a bare Tensor view -- meaning / meter is unknown.
                let meaning = TensorDataMeaning::Unknown;
                let meter = None;
                tensor_summary_ui_grid_contents(ui, tensor, tensor, meaning, meter, &tensor_stats);
            }
        });

        list_item::list_item_scope(ui, "tensor_selection_ui", |ui| {
            view_property_ui::<TensorScalarMapping>(ctx, ui, view_id, self, state);
            view_property_ui::<TensorViewFit>(ctx, ui, view_id, self, state);
        });

        if let Some((_, tensor)) = &state.tensor {
            ui.separator();
            ui.strong("Dimension Mapping");
            dimension_mapping_ui(ui, &mut state.slice.dim_mapping, tensor.shape());
            let default_mapping = DimensionMapping::create(tensor.shape());
            if ui
                .add_enabled(
                    state.slice.dim_mapping != default_mapping,
                    egui::Button::new("Reset mapping"),
                )
                .on_disabled_hover_text("The default is already set up")
                .on_hover_text("Reset dimension mapping to the default")
                .clicked()
            {
                state.slice.dim_mapping = DimensionMapping::create(tensor.shape());
            }
        }

        Ok(())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();
        // For tensors create one space view for each tensor (even though we're able to stack them in one view)
        suggest_space_view_for_each_entity::<TensorSystem>(ctx, self)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();
        let state = state.downcast_mut::<ViewTensorState>()?;

        let tensors = &system_output.view_systems.get::<TensorSystem>()?.tensors;

        if tensors.len() > 1 {
            state.tensor = None;

            egui::Frame {
                inner_margin: re_ui::DesignTokens::view_padding().into(),
                ..egui::Frame::default()
            }
            .show(ui, |ui| {
                ui.label(format!(
                    "Can only show one tensor at a time; was given {}. Update the query so that it \
                    returns a single tensor entity and create additional views for the others.",
                    tensors.len()
                ));
            });
        } else if let Some((tensor_data_row_id, tensor)) = tensors.first() {
            state.tensor = Some((*tensor_data_row_id, tensor.clone()));
            self.view_tensor(
                ctx,
                ui,
                state,
                query.space_view_id,
                *tensor_data_row_id,
                tensor,
            );
        } else {
            state.tensor = None;
            ui.centered_and_justified(|ui| ui.label("(empty)"));
        }

        Ok(())
    }
}

impl TensorSpaceView {
    fn view_tensor(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut ViewTensorState,
        view_id: SpaceViewId,
        tensor_data_row_id: RowId,
        tensor: &DecodedTensor,
    ) {
        re_tracing::profile_function!();

        if !state.slice.dim_mapping.is_valid(tensor.num_dim()) {
            state.slice.dim_mapping = DimensionMapping::create(tensor.shape());
        }

        let default_item_spacing = ui.spacing_mut().item_spacing;
        ui.spacing_mut().item_spacing.y = 0.0; // No extra spacing between sliders and tensor

        if state
            .slice
            .dim_mapping
            .selectors
            .iter()
            .any(|selector| selector.visible)
        {
            egui::Frame {
                inner_margin: egui::Margin::symmetric(16.0, 8.0),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = default_item_spacing; // keep the default spacing between sliders
                selectors_ui(ui, state, tensor);
            });
        }

        let dimension_labels = {
            let dm = &state.slice.dim_mapping;
            [
                (
                    dimension_name(&tensor.shape, dm.width.unwrap_or_default()),
                    dm.invert_width,
                ),
                (
                    dimension_name(&tensor.shape, dm.height.unwrap_or_default()),
                    dm.invert_height,
                ),
            ]
        };

        egui::ScrollArea::both().show(ui, |ui| {
            if let Err(err) = self.tensor_slice_ui(
                ctx,
                ui,
                state,
                view_id,
                tensor_data_row_id,
                tensor,
                dimension_labels,
            ) {
                ui.label(ui.ctx().error_text(err.to_string()));
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn tensor_slice_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &ViewTensorState,
        view_id: SpaceViewId,
        tensor_data_row_id: RowId,
        tensor: &DecodedTensor,
        dimension_labels: [(String, bool); 2],
    ) -> anyhow::Result<()> {
        let (response, painter, image_rect) =
            self.paint_tensor_slice(ctx, ui, state, view_id, tensor_data_row_id, tensor)?;

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
        view_id: SpaceViewId,
        tensor_data_row_id: RowId,
        tensor: &DecodedTensor,
    ) -> anyhow::Result<(egui::Response, egui::Painter, egui::Rect)> {
        re_tracing::profile_function!();

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

        let tensor_stats = ctx
            .cache
            .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));
        let colormapped_texture = super::tensor_slice_to_gpu::colormapped_texture(
            render_ctx,
            tensor_data_row_id,
            tensor,
            &tensor_stats,
            state,
            colormap,
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
        };

        let debug_name = "tensor_slice";
        gpu_bridge::render_image(
            render_ctx,
            &painter,
            image_rect,
            colormapped_texture,
            texture_options,
            debug_name,
        )?;

        Ok((response, painter, image_rect))
    }
}

// ----------------------------------------------------------------------------

pub fn selected_tensor_slice<'a, T: Copy>(
    slice_selection: &SliceSelection,
    tensor: &'a ndarray::ArrayViewD<'_, T>,
) -> ndarray::ArrayViewD<'a, T> {
    let SliceSelection {
        dim_mapping: dimension_mapping,
        selector_values,
    } = slice_selection;

    assert!(dimension_mapping.is_valid(tensor.ndim()));

    let (width, height) =
        if let (Some(width), Some(height)) = (dimension_mapping.width, dimension_mapping.height) {
            (width, height)
        } else if let Some(width) = dimension_mapping.width {
            // If height is missing, create a 1D row.
            (width, 1)
        } else if let Some(height) = dimension_mapping.height {
            // If width is missing, create a 1D column.
            (1, height)
        } else {
            // If both are missing, give up.
            return tensor.view();
        };

    let view = if tensor.shape().len() == 1 {
        // We want 2D slices, so for "pure" 1D tensors add a dimension.
        // This is important for above width/height conversion to work since this assumes at least 2 dimensions.
        tensor
            .view()
            .into_shape(ndarray::IxDyn(&[tensor.len(), 1]))
            .unwrap()
    } else {
        tensor.view()
    };

    #[allow(clippy::tuple_array_conversions)]
    let axis = [height, width]
        .into_iter()
        .chain(dimension_mapping.selectors.iter().map(|s| s.dim_idx))
        .collect::<Vec<_>>();
    let mut slice = view.permuted_axes(axis);

    for DimensionSelector { dim_idx, .. } in &dimension_mapping.selectors {
        let selector_value = selector_values.get(dim_idx).copied().unwrap_or_default() as usize;
        assert!(
            selector_value < slice.shape()[2],
            "Bad tensor slicing. Trying to select slice index {selector_value} of dim=2. tensor shape: {:?}, dim_mapping: {dimension_mapping:#?}",
            tensor.shape()
        );

        // 0 and 1 are width/height, the rest are rearranged by dimension_mapping.selectors
        // This call removes Axis(2), so the next iteration of the loop does the right thing again.
        slice.index_axis_inplace(Axis(2), selector_value);
    }
    if dimension_mapping.invert_height {
        slice.invert_axis(Axis(0));
    }
    if dimension_mapping.invert_width {
        slice.invert_axis(Axis(1));
    }

    slice
}

fn dimension_name(shape: &[TensorDimension], dim_idx: usize) -> String {
    let dim = &shape[dim_idx];
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
    dimension_labels: [(String, bool); 2],
) {
    // Show axis names etc:
    let [(width_name, invert_width), (height_name, invert_height)] = dimension_labels;
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
    {
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
    {
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

fn selectors_ui(ui: &mut egui::Ui, state: &mut ViewTensorState, tensor: &TensorData) {
    for selector in &state.slice.dim_mapping.selectors {
        if !selector.visible {
            continue;
        }

        let dim = &tensor.shape()[selector.dim_idx];
        let size = dim.size;

        let selector_value = state
            .slice
            .selector_values
            .entry(selector.dim_idx)
            .or_insert_with(|| size / 2); // start in the middle

        if size > 0 {
            *selector_value = selector_value.at_most(size - 1);
        }

        if size > 1 {
            ui.horizontal(|ui| {
                let name = dim
                    .name
                    .clone()
                    .map_or_else(|| selector.dim_idx.to_string(), |name| name.to_string());

                let slider_tooltip = format!("Adjust the selected slice for the {name} dimension");
                ui.label(&name).on_hover_text(&slider_tooltip);

                // If the range is big (say, 2048) then we would need
                // a slider that is 2048 pixels wide to get the good precision.
                // So we add a high-precision drag-value instead:
                ui.add(
                    egui::DragValue::new(selector_value)
                        .clamp_range(0..=size - 1)
                        .speed(0.5),
                )
                .on_hover_text(format!(
                    "Drag to precisely control the slice index of the {name} dimension"
                ));

                // Make the slider as big as needed:
                const MIN_SLIDER_WIDTH: f32 = 64.0;
                if ui.available_width() >= MIN_SLIDER_WIDTH {
                    ui.spacing_mut().slider_width = ((size as f32) * 4.0)
                        .at_least(MIN_SLIDER_WIDTH)
                        .at_most(ui.available_width());
                    ui.add(egui::Slider::new(selector_value, 0..=size - 1).show_value(false))
                        .on_hover_text(slider_tooltip);
                }
            });
        }
    }
}

impl TypedComponentFallbackProvider<Colormap> for TensorSpaceView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Colormap {
        // Viridis is a better fallback than Turbo for arbitrary tensors.
        Colormap::Viridis
    }
}

re_viewer_context::impl_component_fallback_provider!(TensorSpaceView => [Colormap]);
