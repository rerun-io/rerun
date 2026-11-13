use re_renderer::renderer::ColormappedTexture;
use re_sdk_types::encodings::TensorData;
use re_view::{
    TextureInteractionId, paint_zoomed_texture_region_outline, zoomed_texture_region_ui,
};
use re_viewer_context::ViewContext;

use crate::dimension_mapping::TensorSliceSelection;
use crate::visualizer_system::TensorVisualization;

/// Shows the index and value of the hovered tensor element, next to a magnified view of the slice around it.
///
/// `response` is the hover response of the view, and the slice texture is stretched over `image_rect`.
pub fn tensor_slice_hover_ui(
    ctx: &ViewContext<'_>,
    ui: &egui::Ui,
    response: egui::Response,
    image_rect: egui::Rect,
    texture: ColormappedTexture,
    tensor_view: &TensorVisualization,
    slice_selection: &TensorSliceSelection,
) {
    let Some(pointer) = response.hover_pos().filter(|pos| image_rect.contains(*pos)) else {
        return;
    };
    let tensor = &tensor_view.tensor;
    let [width, height] = texture.width_height();
    let texel = texel_at(image_rect, pointer, [width, height]);
    let Some(index) = tensor_index_at(slice_selection, tensor.shape(), texel) else {
        return;
    };

    let center_texel = texel.map(i64::from);
    paint_zoomed_texture_region_outline(
        ui.ctx(),
        ui.clip_rect().intersect(response.rect),
        egui::vec2(width as f32, height as f32),
        center_texel,
        image_rect,
    );

    // One hovered slice per view and no GPU readback, so a fixed index is safe.
    let interaction_id = TextureInteractionId {
        entity_path: &tensor_view.entity_path,
        interaction_idx: 0,
    };
    response
        .on_hover_cursor(egui::CursorIcon::Crosshair)
        .on_hover_ui_at_pointer(|ui| {
            ui.horizontal(|ui| {
                zoomed_texture_region_ui(
                    ctx.render_ctx(),
                    ui,
                    texture,
                    &interaction_id,
                    center_texel,
                    &|ui| element_value_ui(ui, tensor, &index),
                );
            });
        });
}

/// Texel of a `[width, height]` texture under `pointer`, clamped to the texture.
///
/// `image_rect` is the screen rect the texture is stretched over, regardless of aspect ratio.
fn texel_at(image_rect: egui::Rect, pointer: egui::Pos2, [width, height]: [u32; 2]) -> [u32; 2] {
    let uv = (pointer - image_rect.min) / image_rect.size();
    let texel = |fraction: f32, size: u32| {
        ((fraction * size as f32).floor() as u32).min(size.saturating_sub(1))
    };
    [texel(uv.x, width), texel(uv.y, height)]
}

/// Index into the full tensor of the element shown at `texel` of the slice texture.
///
/// Returns `None` if the selection refers to dimensions outside `shape`.
fn tensor_index_at(
    slice_selection: &TensorSliceSelection,
    shape: &[u64],
    [x, y]: [u32; 2],
) -> Option<Vec<u64>> {
    let TensorSliceSelection {
        width,
        height,
        indices,
        slider: _,
    } = slice_selection;

    let mut index = vec![0; shape.len()];
    for selection in indices {
        *index.get_mut(selection.dimension as usize)? = selection.index;
    }

    for (selection, texel) in [(width.map(|w| *w), x), (height.map(|h| *h), y)] {
        let Some(selection) = selection else {
            continue;
        };
        let dim = selection.dimension as usize;
        let last = shape.get(dim)?.saturating_sub(1);
        let texel = u64::from(texel).min(last);
        *index.get_mut(dim)? = if selection.invert {
            last - texel
        } else {
            texel
        };
    }

    Some(index)
}

fn element_value_ui(ui: &mut egui::Ui, tensor: &TensorData, index: &[u64]) {
    egui::Grid::new("tensor_element_value").show(ui, |ui| {
        ui.label("Index");
        let index_text = index
            .iter()
            .map(|&i| re_format::format_uint(i))
            .collect::<Vec<_>>()
            .join(", ");
        ui.label(index_text);
        ui.end_row();

        ui.label("Value");
        match tensor.get(index) {
            Some(value) => ui.monospace(value.format_padded()),
            None => ui.label("No value"),
        };
        ui.end_row();
    });
}

#[cfg(test)]
mod tests {
    use re_sdk_types::components::{
        TensorDimensionIndexSelection, TensorHeightDimension, TensorWidthDimension,
    };
    use re_sdk_types::encodings::{self, TensorDimensionSelection};

    use super::*;

    fn selection(
        width: Option<(u32, bool)>,
        height: Option<(u32, bool)>,
        indices: &[(u32, u64)],
    ) -> TensorSliceSelection {
        let dim = |(dimension, invert)| TensorDimensionSelection { dimension, invert };
        TensorSliceSelection {
            width: width.map(|w| TensorWidthDimension(dim(w))),
            height: height.map(|h| TensorHeightDimension(dim(h))),
            indices: indices
                .iter()
                .map(|&(dimension, index)| {
                    TensorDimensionIndexSelection(encodings::TensorDimensionIndexSelection {
                        dimension,
                        index,
                    })
                })
                .collect(),
            slider: None,
        }
    }

    #[test]
    fn texel_under_pointer() {
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(100.0, 50.0));
        // Every texel is 10x10 points.
        assert_eq!(
            texel_at(rect, rect.min + egui::vec2(35.0, 12.0), [10, 5]),
            [3, 1]
        );
        // The far edges clamp to the last texel.
        assert_eq!(texel_at(rect, rect.max, [10, 5]), [9, 4]);
    }

    #[test]
    fn index_2d() {
        let sel = selection(Some((1, false)), Some((0, false)), &[]);
        assert_eq!(tensor_index_at(&sel, &[5, 10], [3, 1]), Some(vec![1, 3]));
    }

    #[test]
    fn index_inverted_with_fixed_dims() {
        let sel = selection(Some((2, true)), Some((0, true)), &[(1, 7)]);
        assert_eq!(
            tensor_index_at(&sel, &[3, 8, 4], [0, 0]),
            Some(vec![2, 7, 3])
        );
    }

    #[test]
    fn index_1d() {
        let width_only = selection(Some((0, false)), None, &[]);
        assert_eq!(tensor_index_at(&width_only, &[10], [5, 0]), Some(vec![5]));
        let height_only = selection(None, Some((0, false)), &[]);
        assert_eq!(tensor_index_at(&height_only, &[10], [0, 5]), Some(vec![5]));
    }
}
