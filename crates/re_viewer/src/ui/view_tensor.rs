use re_log_types::{Tensor, TensorDataType, TensorDimension};

use egui::{Color32, ColorImage};
use itertools::Itertools as _;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TensorViewState {
    /// How we select which dimensions to project the tensor onto.
    rank_mapping: RankMapping,

    /// Maps dimenion to the slice of that dimension.
    selectors: ahash::HashMap<usize, u64>,

    /// How we map values to colors.
    color_mapping: ColorMapping,
}

impl TensorViewState {
    pub(crate) fn create(tensor: &re_log_types::Tensor) -> TensorViewState {
        Self {
            selectors: Default::default(),
            rank_mapping: RankMapping::create(tensor),
            color_mapping: ColorMapping::default(),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct RankMapping {
    /// Which dimensions have selectors?
    selectors: Vec<usize>,

    // Which dim?
    width: Option<usize>,

    // Which dim?
    height: Option<usize>,

    // Which dim?
    channel: Option<usize>,
}

impl RankMapping {
    fn create(tensor: &Tensor) -> RankMapping {
        // TODO(emilk): add a heuristic here for the default
        RankMapping {
            width: Some(1),
            height: Some(0),
            channel: None,
            selectors: (2..tensor.num_dim()).collect(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragDropAddress {
    None,
    Width,
    Height,
    Channel,
    Selector(usize),
    NewSelector,
}

fn tensor_dimension_ui<'a>(
    ui: &mut egui::Ui,
    bound_dim_idx: Option<usize>,
    location: DragDropAddress,
    shape: &[TensorDimension],
    drop_source: &mut DragDropAddress,
    drop_target: &mut DragDropAddress,
) {
    // TODO: don't accept everythingp
    let response = dimension_drop_target(ui, true, |ui| {
        ui.set_min_size(egui::vec2(80., 15.));

        if let Some(dim_idx) = bound_dim_idx.to_owned() {
            let dim = &shape[dim_idx];
            // TODO: Does this need to be globally unique?
            let dim_ui_id = egui::Id::new("tensor_dimension_ui").with(dim_idx);

            let tmp: String;
            let display_name = if dim.name.is_empty() {
                tmp = format!("{}", dim_idx);
                &tmp
            } else {
                &dim.name
            };

            drag_source(ui, dim_ui_id, |ui| {
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

pub fn drag_source(ui: &mut egui::Ui, id: egui::Id, body: impl FnOnce(&mut egui::Ui)) {
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
// TODO: We should make this code reusable.
pub fn dimension_drop_target<R>(
    ui: &mut egui::Ui,
    can_accept_what_is_being_dragged: bool,
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

    let style = if is_being_dragged && can_accept_what_is_being_dragged && response.hovered() {
        ui.visuals().widgets.active
    } else {
        ui.visuals().widgets.inactive
    };

    let mut fill = style.bg_fill;
    let mut stroke = style.bg_stroke;
    if is_being_dragged && !can_accept_what_is_being_dragged {
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

fn rank_mapping_ui(ui: &mut egui::Ui, rank_mapping: &mut RankMapping, shape: &[TensorDimension]) {
    ui.label("TODO");
    ui.monospace(format!("{rank_mapping:?}"));

    let mut drop_source = DragDropAddress::None;
    let mut drop_target = DragDropAddress::None;

    ui.columns(2, |columns| {
        {
            let ui = &mut columns[0];
            ui.heading("Image:");
            egui::Grid::new("imagegrid").num_columns(2).show(ui, |ui| {
                ui.label("Width:");
                tensor_dimension_ui(
                    ui,
                    rank_mapping.width,
                    DragDropAddress::Width,
                    shape,
                    &mut drop_source,
                    &mut drop_target,
                );
                ui.end_row();

                ui.label("Height:");
                tensor_dimension_ui(
                    ui,
                    rank_mapping.height,
                    DragDropAddress::Height,
                    shape,
                    &mut drop_source,
                    &mut drop_target,
                );
                ui.end_row();

                ui.label("Channel:");
                tensor_dimension_ui(
                    ui,
                    rank_mapping.channel,
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
                        rank_mapping.selectors.iter_mut().enumerate()
                    {
                        tensor_dimension_ui(
                            ui,
                            Some(dim_idx),
                            DragDropAddress::Selector(selector_idx),
                            shape,
                            &mut drop_source,
                            &mut drop_target,
                        );
                        ui.end_row();
                    }
                    tensor_dimension_ui(
                        ui,
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
    if drop_target != DragDropAddress::None && drop_source != DragDropAddress::None {
        let read_from_address = |address| match address {
            DragDropAddress::None => unreachable!(),
            DragDropAddress::Width => rank_mapping.width,
            DragDropAddress::Height => rank_mapping.height,
            DragDropAddress::Channel => rank_mapping.channel,
            DragDropAddress::Selector(selector_idx) => Some(rank_mapping.selectors[selector_idx]),
            DragDropAddress::NewSelector => None,
        };
        let previous_value_source = read_from_address(drop_source);
        let previous_value_target = read_from_address(drop_target);

        let mut write_to_address = |address, dim_idx| match address {
            DragDropAddress::None => unreachable!(),
            DragDropAddress::Width => rank_mapping.width = dim_idx,
            DragDropAddress::Height => rank_mapping.height = dim_idx,
            DragDropAddress::Channel => rank_mapping.channel = dim_idx,
            DragDropAddress::Selector(selector_idx) => {
                if let Some(dim_idx) = dim_idx {
                    rank_mapping.selectors[selector_idx] = dim_idx;
                } else {
                    rank_mapping.selectors.remove(selector_idx);
                }
            }
            // NewSelector can only be a drop *target*, therefore dim_idx can't be None!
            DragDropAddress::NewSelector => rank_mapping.selectors.push(dim_idx.unwrap()),
        };
        write_to_address(drop_source, previous_value_target);
        write_to_address(drop_target, previous_value_source);
    }
}

// ----------------------------------------------------------------------------

/// How we map values to colors.
#[derive(Copy, Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ColorMapping {
    turbo: bool,
    gamma: f32,
}

impl Default for ColorMapping {
    fn default() -> Self {
        Self {
            turbo: false,
            gamma: 1.0,
        }
    }
}

impl ColorMapping {
    pub fn color_from_normalized(&self, f: f32) -> Color32 {
        let f = f.powf(self.gamma);

        if self.turbo {
            let [r, g, b] = crate::misc::color_map::turbo_color_map(f);
            Color32::from_rgb(r, g, b)
        } else {
            let lum = (f * 255.0 + 0.5) as u8;
            Color32::from_gray(lum)
        }
    }
}

fn color_mapping_ui(ui: &mut egui::Ui, color_mapping: &mut ColorMapping) {
    ui.horizontal(|ui| {
        ui.label("Color map:");
        let mut brightness = 1.0 / color_mapping.gamma;
        ui.add(
            egui::Slider::new(&mut brightness, 0.1..=10.0)
                .logarithmic(true)
                .text("Brightness"),
        );
        color_mapping.gamma = 1.0 / brightness;
        ui.checkbox(&mut color_mapping.turbo, "Turbo colormap");
    });
}

// ----------------------------------------------------------------------------

pub(crate) fn view_tensor(ui: &mut egui::Ui, state: &mut TensorViewState, tensor: &Tensor) {
    crate::profile_function!();
    ui.heading("Tensor viewer!");
    ui.monospace(format!("shape: {:?}", tensor.shape));
    ui.monospace(format!("dtype: {:?}", tensor.dtype));

    ui.collapsing("Rank Mapping", |ui| {
        rank_mapping_ui(ui, &mut state.rank_mapping, &tensor.shape);
    });
    color_mapping_ui(ui, &mut state.color_mapping);

    selectors_ui(ui, state, tensor);

    let color_mapping = &state.color_mapping;

    match tensor.dtype {
        TensorDataType::U8 => match re_tensor_ops::as_ndarray::<u8>(tensor) {
            Ok(tensor) => {
                let color_from_value =
                    |value: u8| color_mapping.color_from_normalized(value as f32 / 255.0);
                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, &slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::U16 => match re_tensor_ops::as_ndarray::<u16>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = tensor_range_u16(&tensor);
                ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));
                let color_from_value = |value: u16| {
                    color_mapping.color_from_normalized(egui::remap(
                        value as f32,
                        tensor_min as f32..=tensor_max as f32,
                        0.0..=1.0,
                    ))
                };

                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, &slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },

        TensorDataType::F32 => match re_tensor_ops::as_ndarray::<f32>(tensor) {
            Ok(tensor) => {
                let (tensor_min, tensor_max) = tensor_range_f32(&tensor);
                ui.monospace(format!("Data range: [{tensor_min} - {tensor_max}]"));
                let color_from_value = |value: f32| {
                    color_mapping.color_from_normalized(egui::remap(
                        value,
                        tensor_min..=tensor_max,
                        0.0..=1.0,
                    ))
                };

                let slice = tensor.slice(slicer(tensor.ndim(), &state.selectors).as_slice());
                slice_ui(ui, &state.rank_mapping, &slice, color_from_value);
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err.to_string());
            }
        },
    }
}

fn slice_ui<T: Copy>(
    ui: &mut egui::Ui,
    rank_mapping: &RankMapping,
    slice: &ndarray::ArrayViewD<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) {
    ui.monospace(format!("Slice shape: {:?}", slice.shape()));

    if slice.ndim() == 2 {
        let image = into_image(rank_mapping, slice, color_from_value);
        image_ui(ui, image);
    } else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            "Only 2D slices supported at the moment",
        );
    }
}

fn into_image<T: Copy>(
    rank_mapping: &RankMapping,
    slice: &ndarray::ArrayViewD<'_, T>,
    color_from_value: impl Fn(T) -> Color32,
) -> ColorImage {
    crate::profile_function!();
    assert_eq!(slice.ndim(), 2);
    // what is height or what is width depends on the rank-mapping
    if rank_mapping.height < rank_mapping.width {
        let (height, width) = (slice.shape()[0], slice.shape()[1]);
        let mut image = egui::ColorImage::new([width, height], Color32::DEBUG_COLOR);
        assert_eq!(image.pixels.len(), slice.iter().count());
        crate::profile_scope!("color_mapper");
        for (pixel, value) in itertools::izip!(&mut image.pixels, slice) {
            *pixel = color_from_value(*value);
        }
        image
    } else {
        // transpose:
        let (width, height) = (slice.shape()[0], slice.shape()[1]);
        let mut image = egui::ColorImage::new([width, height], Color32::DEBUG_COLOR);
        assert_eq!(image.pixels.len(), slice.iter().count());
        crate::profile_scope!("color_mapper_transposed");
        for y in 0..height {
            for x in 0..width {
                image[(x, y)] = color_from_value(slice[[x, y]]);
            }
        }
        image
    }
}

fn image_ui(ui: &mut egui::Ui, image: ColorImage) {
    crate::profile_function!();
    // TODO(emilk): cache texture - don't create a new texture every frame
    let texture = ui
        .ctx()
        .load_texture("tensor_slice", image, egui::TextureFilter::Linear);
    egui::ScrollArea::both().show(ui, |ui| {
        ui.image(texture.id(), texture.size_vec2());
    });
}

fn selectors_ui(ui: &mut egui::Ui, state: &mut TensorViewState, tensor: &Tensor) {
    for &dim_idx in &state.rank_mapping.selectors {
        let dim = &tensor.shape[dim_idx];
        let name = if dim.name.is_empty() {
            dim_idx.to_string()
        } else {
            dim.name.clone()
        };
        let len = dim.size;
        if len > 1 {
            let slice = state.selectors.entry(dim_idx).or_default();
            ui.add(egui::Slider::new(slice, 0..=len - 1).text(name));
        }
    }
}

fn tensor_range_f32(tensor: &ndarray::ArrayViewD<'_, f32>) -> (f32, f32) {
    crate::profile_function!();
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &value in tensor {
        min = min.min(value);
        max = max.max(value);
    }
    (min, max)
}

fn tensor_range_u16(tensor: &ndarray::ArrayViewD<'_, u16>) -> (u16, u16) {
    crate::profile_function!();
    let mut min = u16::MAX;
    let mut max = u16::MIN;
    for &value in tensor {
        min = min.min(value);
        max = max.max(value);
    }
    (min, max)
}

fn slicer(num_dim: usize, selectors: &ahash::HashMap<usize, u64>) -> Vec<ndarray::SliceInfoElem> {
    (0..num_dim)
        .map(|dim| {
            if let Some(selector) = selectors.get(&dim) {
                ndarray::SliceInfoElem::Index(*selector as _)
            } else {
                ndarray::SliceInfoElem::Slice {
                    start: 0,
                    end: None,
                    step: 1,
                }
            }
        })
        .collect_vec()
}
