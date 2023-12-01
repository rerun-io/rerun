use std::{collections::BTreeMap, fmt::Display};

use egui::{epaint::TextShape, Align2, NumExt as _, Vec2};
use ndarray::Axis;
use re_data_store::EntityProperties;

use re_data_ui::tensor_summary_ui_grid_contents;
use re_log_types::{EntityPath, RowId};
use re_renderer::Colormap;
use re_tensor_ops::dimension_mapping::{DimensionMapping, DimensionSelector};
use re_types::{
    datatypes::{TensorData, TensorDimension},
    tensor_data::{DecodedTensor, TensorDataMeaning},
};
use re_viewer_context::{
    gpu_bridge, gpu_bridge::colormap_dropdown_button_ui, SpaceViewClass,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewSystemExecutionError,
    TensorStatsCache, ViewContextCollection, ViewPartCollection, ViewQuery, ViewerContext,
};

use crate::{tensor_dimension_mapper::dimension_mapping_ui, view_part_system::TensorSystem};

#[derive(Default)]
pub struct TensorSpaceView;

#[derive(Default)]
pub struct ViewTensorState {
    /// Selects in [`Self::state_tensors`].
    pub selected_tensor: Option<EntityPath>,

    pub state_tensors: ahash::HashMap<EntityPath, PerTensorState>,
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

pub struct PerTensorState {
    /// What slice are we vieiwing?
    slice: SliceSelection,

    /// How we map values to colors.
    color_mapping: ColorMapping,

    /// Scaling, filtering, aspect ratio, etc for the rendered texture.
    texture_settings: TextureSettings,

    /// Last viewed tensor, copied each frame.
    /// Used for the selection view.
    tensor: Option<(RowId, DecodedTensor)>,
}

impl PerTensorState {
    pub fn create(tensor_data_row_id: RowId, tensor: &DecodedTensor) -> PerTensorState {
        Self {
            slice: SliceSelection {
                dim_mapping: DimensionMapping::create(tensor.shape()),
                selector_values: Default::default(),
            },
            color_mapping: ColorMapping::default(),
            texture_settings: TextureSettings::default(),
            tensor: Some((tensor_data_row_id, tensor.clone())),
        }
    }

    pub fn slice(&self) -> &SliceSelection {
        &self.slice
    }

    pub fn color_mapping(&self) -> &ColorMapping {
        &self.color_mapping
    }

    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        let Some((tensor_data_row_id, tensor)) = &self.tensor else {
            ui.label("No Tensor shown in this Space View.");
            return;
        };

        let tensor_stats = ctx
            .cache
            .entry(|c: &mut TensorStatsCache| c.entry(*tensor_data_row_id, tensor));
        ctx.re_ui
            .selection_grid(ui, "tensor_selection_ui")
            .show(ui, |ui| {
                // We are in a bare Tensor view -- meaning / meter is unknown.
                let meaning = TensorDataMeaning::Unknown;
                let meter = None;
                tensor_summary_ui_grid_contents(
                    ctx.re_ui,
                    ui,
                    tensor,
                    tensor,
                    meaning,
                    meter,
                    &tensor_stats,
                );
                self.texture_settings.ui(ctx.re_ui, ui);
                self.color_mapping.ui(ctx.render_ctx, ctx.re_ui, ui);
            });

        ui.separator();
        ui.strong("Dimension Mapping");
        dimension_mapping_ui(ctx.re_ui, ui, &mut self.slice.dim_mapping, tensor.shape());
        let default_mapping = DimensionMapping::create(tensor.shape());
        if ui
            .add_enabled(
                self.slice.dim_mapping != default_mapping,
                egui::Button::new("Reset mapping"),
            )
            .on_disabled_hover_text("The default is already set up")
            .on_hover_text("Reset dimension mapping to the default")
            .clicked()
        {
            self.slice.dim_mapping = DimensionMapping::create(tensor.shape());
        }
    }
}

impl SpaceViewClass for TensorSpaceView {
    type State = ViewTensorState;

    const IDENTIFIER: &'static str = "Tensor";
    const DISPLAY_NAME: &'static str = "Tensor";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TENSOR
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "Select the Space View to configure which dimensions are shown.".into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_part_system::<TensorSystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Medium
    }

    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
        if let Some(selected_tensor) = &state.selected_tensor {
            if let Some(state_tensor) = state.state_tensors.get_mut(selected_tensor) {
                state_tensor.ui(ctx, ui);
            }
        }
    }

    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        _view_ctx: &ViewContextCollection,
        parts: &ViewPartCollection,
        _query: &ViewQuery<'_>,
        _draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let tensors = &parts.get::<TensorSystem>()?.tensors;

        if tensors.is_empty() {
            ui.centered_and_justified(|ui| ui.label("(empty)"));
            state.selected_tensor = None;
        } else {
            if let Some(selected_tensor) = &state.selected_tensor {
                if !tensors.contains_key(selected_tensor) {
                    state.selected_tensor = None;
                }
            }
            if state.selected_tensor.is_none() {
                state.selected_tensor = Some(tensors.iter().next().unwrap().0.clone());
            }

            if tensors.len() > 1 {
                // Show radio buttons for the different tensors we have in this view - better than nothing!
                ui.horizontal(|ui| {
                    for instance_path in tensors.keys() {
                        let is_selected = state.selected_tensor.as_ref() == Some(instance_path);
                        if ui.radio(is_selected, instance_path.to_string()).clicked() {
                            state.selected_tensor = Some(instance_path.clone());
                        }
                    }
                });
            }

            if let Some(selected_tensor) = &state.selected_tensor {
                if let Some((tensor_data_row_id, tensor)) = tensors.get(selected_tensor) {
                    let state_tensor = state
                        .state_tensors
                        .entry(selected_tensor.clone())
                        .or_insert_with(|| PerTensorState::create(*tensor_data_row_id, tensor));
                    view_tensor(ctx, ui, state_tensor, *tensor_data_row_id, tensor);
                }
            }
        }

        Ok(())
    }
}

fn view_tensor(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut PerTensorState,
    tensor_data_row_id: RowId,
    tensor: &DecodedTensor,
) {
    re_tracing::profile_function!();

    state.tensor = Some((tensor_data_row_id, tensor.clone()));

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
        if let Err(err) =
            tensor_slice_ui(ctx, ui, state, tensor_data_row_id, tensor, dimension_labels)
        {
            ui.label(ctx.re_ui.error_text(err.to_string()));
        }
    });
}

fn tensor_slice_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &PerTensorState,
    tensor_data_row_id: RowId,
    tensor: &DecodedTensor,
    dimension_labels: [(String, bool); 2],
) -> anyhow::Result<()> {
    let (response, painter, image_rect) =
        paint_tensor_slice(ctx, ui, state, tensor_data_row_id, tensor)?;

    if !response.hovered() {
        let font_id = egui::TextStyle::Body.resolve(ui.style());
        paint_axis_names(ui, &painter, image_rect, font_id, dimension_labels);
    }

    Ok(())
}

fn paint_tensor_slice(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &PerTensorState,
    tensor_data_row_id: RowId,
    tensor: &DecodedTensor,
) -> anyhow::Result<(egui::Response, egui::Painter, egui::Rect)> {
    re_tracing::profile_function!();

    let tensor_stats = ctx
        .cache
        .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));
    let colormapped_texture = super::tensor_slice_to_gpu::colormapped_texture(
        ctx.render_ctx,
        tensor_data_row_id,
        tensor,
        &tensor_stats,
        state,
    )?;
    let [width, height] = colormapped_texture.width_height();

    let img_size = egui::vec2(width as _, height as _);
    let img_size = Vec2::max(Vec2::splat(1.0), img_size); // better safe than sorry
    let desired_size = match state.texture_settings.scaling {
        TextureScaling::Original => img_size,
        TextureScaling::Fill => {
            let desired_size = ui.available_size();
            if state.texture_settings.keep_aspect_ratio {
                let scale = (desired_size / img_size).min_elem();
                img_size * scale
            } else {
                desired_size
            }
        }
    };

    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
    let rect = response.rect;
    let image_rect = egui::Rect::from_min_max(rect.min, rect.max);

    let debug_name = "tensor_slice";
    gpu_bridge::render_image(
        ctx.render_ctx,
        &painter,
        image_rect,
        colormapped_texture,
        state.texture_settings.options,
        debug_name,
    )?;

    Ok((response, painter, image_rect))
}

// ----------------------------------------------------------------------------

/// How we map values to colors.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ColorMapping {
    pub map: Colormap,
    pub gamma: f32,
}

impl Default for ColorMapping {
    fn default() -> Self {
        Self {
            map: Colormap::Viridis,
            gamma: 1.0,
        }
    }
}

impl ColorMapping {
    fn ui(
        &mut self,
        render_ctx: &mut re_renderer::RenderContext,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
    ) {
        let ColorMapping { map, gamma } = self;

        re_ui.grid_left_hand_label(ui, "Color map");
        colormap_dropdown_button_ui(render_ctx, ui, map);
        ui.end_row();

        re_ui.grid_left_hand_label(ui, "Brightness");
        let mut brightness = 1.0 / *gamma;
        ui.add(egui::Slider::new(&mut brightness, 0.1..=10.0).logarithmic(true));
        *gamma = 1.0 / brightness;
        ui.end_row();
    }
}

// ----------------------------------------------------------------------------

/// Should we scale the rendered texture, and if so, how?
#[derive(Copy, Clone, Debug, PartialEq)]
enum TextureScaling {
    /// No scaling, texture size will match the tensor's width/height dimensions.
    Original,

    /// Scale the texture for the largest possible fit in the UI container.
    Fill,
}

impl Default for TextureScaling {
    fn default() -> Self {
        Self::Fill
    }
}

impl Display for TextureScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureScaling::Original => "Original".fmt(f),
            TextureScaling::Fill => "Fill".fmt(f),
        }
    }
}

/// Scaling, filtering, aspect ratio, etc for the rendered texture.
#[derive(Copy, Clone, Debug, PartialEq)]
struct TextureSettings {
    /// Should the aspect ratio of the tensor be kept when scaling?
    keep_aspect_ratio: bool,

    /// Should we scale the texture when rendering?
    scaling: TextureScaling,

    /// Specifies the sampling filter used to render the texture.
    options: egui::TextureOptions,
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            keep_aspect_ratio: true,
            scaling: TextureScaling::default(),
            options: egui::TextureOptions {
                // This is best for low-res depth-images and the like
                magnification: egui::TextureFilter::Nearest,
                minification: egui::TextureFilter::Linear,
            },
        }
    }
}

// ui
impl TextureSettings {
    fn ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        let TextureSettings {
            keep_aspect_ratio,
            scaling,
            options,
        } = self;

        re_ui.grid_left_hand_label(ui, "Scale");
        ui.vertical(|ui| {
            egui::ComboBox::from_id_source("texture_scaling")
                .selected_text(scaling.to_string())
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(64.0);

                    let mut selectable_value =
                        |ui: &mut egui::Ui, e| ui.selectable_value(scaling, e, e.to_string());
                    selectable_value(ui, TextureScaling::Original);
                    selectable_value(ui, TextureScaling::Fill);
                });
            if *scaling == TextureScaling::Fill {
                re_ui.checkbox(ui, keep_aspect_ratio, "Keep aspect ratio");
            }
        });
        ui.end_row();

        re_ui
            .grid_left_hand_label(ui, "Filtering")
            .on_hover_text("Filtering to use when magnifying");

        fn tf_to_string(tf: egui::TextureFilter) -> &'static str {
            match tf {
                egui::TextureFilter::Nearest => "Nearest",
                egui::TextureFilter::Linear => "Linear",
            }
        }
        egui::ComboBox::from_id_source("texture_filter")
            .selected_text(tf_to_string(options.magnification))
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.set_min_width(64.0);

                let mut selectable_value = |ui: &mut egui::Ui, e| {
                    ui.selectable_value(&mut options.magnification, e, tf_to_string(e))
                };
                selectable_value(ui, egui::TextureFilter::Nearest);
                selectable_value(ui, egui::TextureFilter::Linear);
            });
        ui.end_row();
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

    // TODO(andreas) - shouldn't just give up here
    if dimension_mapping.width.is_none() || dimension_mapping.height.is_none() {
        return tensor.view();
    }

    let axis = dimension_mapping
        .height
        .into_iter()
        .chain(dimension_mapping.width)
        .chain(dimension_mapping.selectors.iter().map(|s| s.dim_idx))
        .collect::<Vec<_>>();
    let mut slice = tensor.view().permuted_axes(axis);

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

    let rounding = re_ui::ReUi::normal_rounding();
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
            painter.add(TextShape {
                pos,
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
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
            painter.add(TextShape {
                pos,
                galley,
                angle: -std::f32::consts::TAU / 4.0,
                underline: Default::default(),
                override_text_color: None,
            });
            egui::Rect::from_min_size(
                pos - galley_size.x * egui::Vec2::Y,
                egui::vec2(galley_size.y, galley_size.x),
            )
        };
        paint_text_bg(text_background, text_rect);
    }
}

fn selectors_ui(ui: &mut egui::Ui, state: &mut PerTensorState, tensor: &TensorData) {
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
