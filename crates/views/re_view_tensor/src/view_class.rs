use egui::epaint::TextShape;
use egui::{Align2, NumExt as _, Pos2, Rect, Vec2};
use ndarray::Axis;
use re_data_ui::tensor_summary_ui_grid_contents;
use re_log_types::EntityPath;
use re_renderer::renderer::ColormappedTexture;
use re_sdk_types::blueprint::archetypes::{
    self, TensorScalarMapping, TensorViewFit, VisualBounds2D,
};
use re_sdk_types::blueprint::components::{self as blueprint_components, ViewFit};
use re_sdk_types::components::{
    Colormap, GammaCorrection, MagnificationFilter, TensorDimensionIndexSelection,
};
use re_sdk_types::encodings::TensorData;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, IconText, MouseButtonText, UiExt as _, icons, list_item};
use re_view::controls::DRAG_PAN2D_BUTTON;
use re_view::{VisualBoundsFit, pan_zoom_visual_bounds_2d, view_property_ui};
use re_viewer_context::{
    ColormapWithRange, IdentifiedViewSystem as _, Item, SystemCommand, SystemCommandSender as _,
    TensorStatsCache, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewContext, ViewId,
    ViewQuery, ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext, gpu_bridge,
    suggest_view_for_each_entity,
};
use re_viewport_blueprint::ViewProperty;

use crate::TensorDimension;
use crate::dimension_mapping::TensorSliceSelection;
use crate::tensor_dimension_mapper::dimension_mapping_ui;
use crate::tensor_slice_hover::tensor_slice_hover_ui;
use crate::visualizer_system::{TensorSystem, TensorVisualization};

#[derive(Default)]
pub struct TensorView;

type ViewType = re_sdk_types::blueprint::views::TensorView;

#[derive(Default, re_byte_size::SizeBytes)]
pub struct ViewTensorState {
    /// Last viewed tensor, copied each frame.
    /// Used for the selection view.
    tensor: Option<TensorVisualization>,

    /// Fallback for the [`VisualBounds2D`] of the view, in element coordinates of the shown slice.
    default_visual_bounds: Option<Rect>,

    /// Layout of the last shown slice; the visual bounds are cleared when it changes.
    slice_layout: Option<SliceLayout>,
}

/// The slice dimensions and fit mode that the visual bounds were set for.
#[derive(Clone, Copy, PartialEq, Eq, re_byte_size::SizeBytes)]
struct SliceLayout {
    width_dimension: Option<u32>,
    height_dimension: Option<u32>,
    view_fit: ViewFit,
}

impl ViewState for ViewTensorState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn heap_size_bytes(&self) -> u64 {
        re_byte_size::SizeBytes::heap_size_bytes(self)
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

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        let egui::InputOptions { zoom_modifier, .. } = egui::InputOptions::default(); // This is OK, since we don't allow the user to change this modifier.

        Help::new("Tensor view")
            .docs_link("https://rerun.io/docs/reference/types/views/tensor_view")
            .markdown(
                "An N-dimensional tensor displayed as a 2D slice with a custom colormap.

Set the displayed dimensions in a selection panel.",
            )
            .control("Pan", (MouseButtonText(DRAG_PAN2D_BUTTON), "+", "drag"))
            .control(
                "Zoom",
                IconText::from_modifiers_and(os, zoom_modifier, icons::SCROLL),
            )
            .control("Reset view", ("double", icons::LEFT_MOUSE_CLICK))
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_fallback_provider(
            TensorScalarMapping::descriptor_colormap().component,
            |_| Colormap::Viridis,
        );
        system_registry.register_fallback_provider(
            VisualBounds2D::descriptor_range().component,
            |ctx| {
                ctx.view_state()
                    .downcast_ref::<ViewTensorState>()
                    .ok()
                    .and_then(|state| state.default_visual_bounds)
                    .map(blueprint_components::VisualBounds2D::from)
                    .unwrap_or_default()
            },
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

    fn selection_ui<'a>(
        &'a self,
        _view_ctx: &re_viewer_context::ViewContext<'_>,
    ) -> re_viewer_context::ViewSelectionUi<'a> {
        re_viewer_context::ViewSelectionUi::properties_ui(move |ui, ctx| {
            let state = ctx.view_state.downcast_ref::<ViewTensorState>()?;

            // TODO(andreas): Listitemify
            ui.selection_grid("tensor_selection_ui").show(ui, |ui| {
                if let Some(TensorVisualization {
                    tensor,
                    tensor_row_id,
                    tensor_source,
                    ..
                }) = &state.tensor
                {
                    let tensor_stats =
                        ctx.viewer_ctx
                            .store_context
                            .memoizer(|c: &mut TensorStatsCache| {
                                c.entry(*tensor_row_id, tensor_source, tensor)
                            });

                    tensor_summary_ui_grid_contents(ui, tensor, &tensor_stats);
                }
            });

            list_item::list_item_scope(ui, "tensor_selection_ui", |ui| {
                view_property_ui::<TensorScalarMapping>(ctx, ui);
                view_property_ui::<TensorViewFit>(ctx, ui);
                view_property_ui::<VisualBounds2D>(ctx, ui);
            });

            // TODO(#6075): Listitemify
            if let Some(TensorVisualization { tensor, .. }) = &state.tensor {
                let slice_property = ViewProperty::from_archetype_for_view::<
                    re_sdk_types::blueprint::archetypes::TensorSliceSelection,
                >(ctx.viewer_ctx, ctx.view_id);
                let slice_selection = TensorSliceSelection::load_and_make_valid(
                    &slice_property,
                    &TensorDimension::from_tensor_data(tensor),
                )?;

                ui.separator();
                ui.strong("Dimension Mapping");
                dimension_mapping_ui(
                    ctx.viewer_ctx,
                    ui,
                    &TensorDimension::from_tensor_data(tensor),
                    &slice_selection,
                    &slice_property,
                );

                // TODO(andreas): this is a bit too inconsistent with the other UIs - we don't offer the same reset/option buttons here
                if ui
                    .button("Reset to default blueprint")
                    .on_hover_text(
                        "Reset dimension mapping to the previously set default blueprint",
                    )
                    .clicked()
                {
                    slice_property.reset_all_components(ctx.viewer_ctx);
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
                    slice_property.reset_all_components_to_empty(ctx.viewer_ctx);
                }
            }

            Ok(())
        })
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        spawn_context: re_viewer_context::ViewSpawnContext,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        // For tensors create one view for each tensor (even though we're able to stack them in one view)
        suggest_view_for_each_entity::<TensorSystem>(ctx, spawn_context)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<re_viewer_context::ViewClassUiOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let tokens = ui.tokens();

        let state = state.downcast_mut::<ViewTensorState>()?;
        state.tensor = None;

        let tensors = system_output
            .visualizer_data_or_default::<Vec<TensorVisualization>>(TensorSystem::identifier())?;

        let response = {
            let mut ui = ui.new_child(egui::UiBuilder::new().sense(egui::Sense::click()));

            if tensors.len() > 1 {
                egui::Frame {
                    inner_margin: tokens.view_padding().into(),
                    ..egui::Frame::default()
                }
                    .show(&mut ui, |ui| {
                        ui.error_label(format!(
                            "Can only show one tensor at a time; was given {}. Update the query so that it \
                    returns a single tensor entity and create additional views for the others.",
                            tensors.len()
                        ));
                    });
            } else if let Some(tensor_view) = tensors.first() {
                state.tensor = Some(tensor_view.clone());
                let (slice_layout, default_visual_bounds) = self.view_tensor(
                    ctx,
                    &mut ui,
                    state,
                    query.view_id,
                    query.space_origin,
                    &tensor_view.tensor,
                )?;

                let previous_layout = state.slice_layout.replace(slice_layout);
                if previous_layout.is_some_and(|previous| previous != slice_layout) {
                    ViewProperty::from_archetype_for_view::<VisualBounds2D>(ctx, query.view_id)
                        .clear_blueprint_component(ctx, VisualBounds2D::descriptor_range());
                }
                if default_visual_bounds.is_some() {
                    state.default_visual_bounds = default_visual_bounds;
                }
            } else {
                ui.centered_and_justified(|ui| ui.label("(empty)"));
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

        Ok(Default::default())
    }
}

impl TensorView {
    fn view_tensor(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &ViewTensorState,
        view_id: ViewId,
        space_origin: &EntityPath,
        tensor: &TensorData,
    ) -> Result<(SliceLayout, Option<Rect>), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let slice_property = ViewProperty::from_archetype_for_view::<
            re_sdk_types::blueprint::archetypes::TensorSliceSelection,
        >(ctx, view_id);
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

        let view_ctx = self.view_context(ctx, view_id, state, space_origin);
        let view_fit: ViewFit = ViewProperty::from_archetype::<TensorViewFit>(&view_ctx)
            .component_or_fallback(&view_ctx, TensorViewFit::descriptor_scaling().component)?;
        let slice_layout = SliceLayout {
            width_dimension: slice_selection.width.map(|width| width.dimension),
            height_dimension: slice_selection.height.map(|height| height.dimension),
            view_fit,
        };

        let default_visual_bounds = match Self::tensor_slice_ui(
            &view_ctx,
            ui,
            state,
            dimension_labels,
            &slice_selection,
            view_fit,
        ) {
            Ok(default_visual_bounds) => Some(default_visual_bounds),
            Err(err) => {
                ui.error_label(err.to_string());
                None
            }
        };

        Ok((slice_layout, default_visual_bounds))
    }

    fn tensor_slice_ui(
        ctx: &ViewContext<'_>,
        ui: &mut egui::Ui,
        state: &ViewTensorState,
        dimension_labels: [Option<(String, bool)>; 2],
        slice_selection: &TensorSliceSelection,
        view_fit: ViewFit,
    ) -> anyhow::Result<Rect> {
        let PaintedSlice {
            response,
            texture,
            image_rect,
            default_visual_bounds,
        } = Self::paint_tensor_slice(ctx, ui, state, slice_selection, view_fit)?;

        // The slice covers the whole view and senses clicks and drags, which takes them from the view's own response.
        if response.hovered() {
            ctx.viewer_ctx
                .selection_state()
                .set_hovered(Item::View(ctx.view_id));
        }
        if response.clicked() {
            ctx.viewer_ctx
                .command_sender()
                .send_system(SystemCommand::set_selection(Item::View(ctx.view_id)));
        }

        if !response.hovered() {
            let font_id = egui::TextStyle::Body.resolve(ui.style());
            paint_axis_names(ui, response.rect, &font_id, dimension_labels);
        } else if let Some(tensor_view) = &state.tensor {
            tensor_slice_hover_ui(
                ctx,
                ui,
                response,
                image_rect,
                texture,
                tensor_view,
                slice_selection,
            );
        }

        Ok(default_visual_bounds)
    }

    fn paint_tensor_slice(
        ctx: &ViewContext<'_>,
        ui: &mut egui::Ui,
        state: &ViewTensorState,
        slice_selection: &TensorSliceSelection,
        view_fit: ViewFit,
    ) -> anyhow::Result<PaintedSlice> {
        re_tracing::profile_function!();

        let Some(tensor_view) = state.tensor.as_ref() else {
            anyhow::bail!("No tensor data available.");
        };
        let TensorVisualization {
            entity_path: _,
            tensor_row_id,
            tensor_source: _,
            tensor,
            data_range,
        } = &tensor_view;

        let scalar_mapping = ViewProperty::from_archetype::<TensorScalarMapping>(ctx);
        let colormap: Colormap = scalar_mapping
            .component_or_fallback(ctx, TensorScalarMapping::descriptor_colormap().component)?;
        let gamma: GammaCorrection = scalar_mapping
            .component_or_fallback(ctx, TensorScalarMapping::descriptor_gamma().component)?;
        let mag_filter: MagnificationFilter = scalar_mapping
            .component_or_fallback(ctx, TensorScalarMapping::descriptor_mag_filter().component)?;

        let colormap = ColormapWithRange {
            colormap,
            value_range: data_range.try_as_f32_range().ok_or_else(|| {
                anyhow::anyhow!("value_range must have finite, distinct f32 endpoints")
            })?,
        };
        let colormapped_texture = super::tensor_slice_to_gpu::colormapped_texture(
            ctx.render_ctx(),
            *tensor_row_id,
            tensor,
            slice_selection,
            &colormap,
            gamma,
        )?;
        let [width, height] = colormapped_texture.width_height();
        let slice_rect = Rect::from_min_size(
            Pos2::ZERO,
            Vec2::max(Vec2::splat(1.0), egui::vec2(width as _, height as _)),
        );

        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

        // In `Original`, one element covers one point, starting at the top left corner of the view.
        let default_visual_bounds = match view_fit {
            ViewFit::Original => Rect::from_min_size(
                Pos2::ZERO,
                Vec2::max(Vec2::splat(1.0), response.rect.size()),
            ),
            ViewFit::Fill | ViewFit::FillKeepAspectRatio => slice_rect,
        };
        // The registered fallback is only updated after this frame, and `Original` depends on the view size,
        // so the default bounds are applied here instead.
        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(ctx);
        let bounds = bounds_property
            .component_or_empty::<blueprint_components::VisualBounds2D>(
                VisualBounds2D::descriptor_range().component,
            )?
            .unwrap_or_else(|| default_visual_bounds.into());
        let ui_from_scene = pan_zoom_visual_bounds_2d(
            ctx.viewer_ctx,
            &response,
            &bounds_property,
            bounds,
            match view_fit {
                ViewFit::Fill => VisualBoundsFit::Stretch,
                ViewFit::FillKeepAspectRatio | ViewFit::Original => VisualBoundsFit::Letterbox,
            },
            Some(slice_rect.size()),
        );
        let image_rect = ui_from_scene.transform_rect(slice_rect);
        let texture_options = egui::TextureOptions {
            magnification: match mag_filter {
                MagnificationFilter::Nearest => egui::TextureFilter::Nearest,
                MagnificationFilter::Linear | MagnificationFilter::Bicubic => {
                    egui::TextureFilter::Linear
                }
            },
            minification: egui::TextureFilter::Linear, // TODO(andreas): allow for mipmapping based filter
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: None,
        };

        gpu_bridge::render_image(
            ctx.render_ctx(),
            &painter,
            image_rect,
            colormapped_texture.clone(),
            texture_options,
            ctx.view_id.render_view_id(),
            re_renderer::Label::from("tensor_slice"),
        )?;

        Ok(PaintedSlice {
            response,
            texture: colormapped_texture,
            image_rect,
            default_visual_bounds,
        })
    }
}

struct PaintedSlice {
    /// Covers the whole view, which `image_rect` may only partially overlap.
    response: egui::Response,

    texture: ColormappedTexture,

    /// The screen rect the slice texture is stretched over.
    image_rect: Rect,

    default_visual_bounds: Rect,
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

    let axis = std::iter::chain(
        [dheight as usize, dwidth as usize],
        indices.iter().map(|s| s.dimension as usize),
    )
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
    rect: egui::Rect,
    font_id: &egui::FontId,
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
            paint_axis_name(ui, pos, align, 0.0, false, width_name, font_id, text_color)
        } else {
            // On right, pointing right:
            let (pos, align) = if invert_height {
                (rect.right_bottom(), Align2::RIGHT_BOTTOM)
            } else {
                (rect.right_top(), Align2::RIGHT_TOP)
            };
            paint_axis_name(ui, pos, align, 0.0, true, width_name, font_id, text_color)
        };
        paint_text_bg(text_background, text_rect);
    }

    // Label for Y axis:
    if let Some(height_name) = height_name {
        let text_background = painter.add(egui::Shape::Noop);
        let angle = -std::f32::consts::TAU / 4.0;
        let text_rect = if invert_height {
            // On top, pointing up:
            let (pos, align) = if invert_width {
                (rect.right_top(), Align2::RIGHT_TOP)
            } else {
                (rect.left_top(), Align2::LEFT_TOP)
            };
            paint_axis_name(
                ui,
                pos,
                align,
                angle,
                true,
                height_name,
                font_id,
                text_color,
            )
        } else {
            // On bottom, pointing down:
            let (pos, align) = if invert_width {
                (rect.right_bottom(), Align2::RIGHT_BOTTOM)
            } else {
                (rect.left_bottom(), Align2::LEFT_BOTTOM)
            };
            paint_axis_name(
                ui,
                pos,
                align,
                angle,
                false,
                height_name,
                font_id,
                text_color,
            )
        };
        paint_text_bg(text_background, text_rect);
    }
}

/// Paints one axis name with an arrow beside it, and returns the rect the two cover.
///
/// The label is laid out left-to-right and then rotated by `angle` around its own origin,
/// with `align` placing the result against `anchor`.
/// `forward` puts the arrow ahead of the name, pointing along the reading direction;
/// otherwise the arrow follows the name and points back.
///
/// The arrow is an icon rather than a glyph: no font we bundle carries the arrow characters,
/// and the viewer must not depend on a system font that may be missing.
#[expect(clippy::too_many_arguments)]
fn paint_axis_name(
    ui: &egui::Ui,
    anchor: egui::Pos2,
    align: Align2,
    angle: f32,
    forward: bool,
    name: String,
    font_id: &egui::FontId,
    color: egui::Color32,
) -> egui::Rect {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(name, font_id.clone(), color);
    let text_size = galley.size();
    let arrow_side = text_size.y;
    let gap = 0.25 * arrow_side;
    let size = egui::vec2(arrow_side + gap + text_size.x, text_size.y);

    let quarter_turn = angle != 0.0;
    let rect = align.anchor_size(anchor, if quarter_turn { size.yx() } else { size });

    // `angle` maps the label's local axes onto the screen, so the local origin is
    // whichever corner of `rect` local (0, 0) rotates onto.
    let origin = if quarter_turn {
        rect.left_bottom()
    } else {
        rect.left_top()
    };
    let rotation = egui::emath::Rot2::from_angle(angle);
    let to_screen = |local: egui::Vec2| origin + rotation * local;

    let text_x = if forward { arrow_side + gap } else { 0.0 };
    painter
        .add(TextShape::new(to_screen(egui::vec2(text_x, 0.0)), galley, color).with_angle(angle));

    let arrow_x = if forward { 0.0 } else { text_size.x + gap };
    let arrow_center = egui::vec2(arrow_x + 0.5 * arrow_side, 0.5 * arrow_side);
    let arrow_rect =
        egui::Rect::from_center_size(to_screen(arrow_center), egui::Vec2::splat(arrow_side));
    // The icon already points along the reading direction, so only the backwards case
    // needs a half turn, before `angle` turns the whole label.
    let arrow_angle = if forward { 0.0 } else { std::f32::consts::PI } + angle;
    re_ui::icons::ARROW_RIGHT
        .as_image()
        .tint(color)
        .rotate(arrow_angle, egui::Vec2::splat(0.5))
        .paint_at(ui, arrow_rect);

    rect
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

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| TensorView.help(ctx));
}
