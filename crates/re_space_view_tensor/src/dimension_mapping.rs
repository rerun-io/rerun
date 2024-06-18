use egui::NumExt as _;
use re_types::{
    blueprint::{archetypes::TensorSliceSelection, components::TensorDimensionIndexSlider},
    components::{TensorDimensionIndexSelection, TensorHeightDimension, TensorWidthDimension},
    datatypes::{TensorDimension, TensorDimensionSelection},
};
use re_viewport_blueprint::ViewProperty;

/// Loads slice selection from blueprint and makes modifications (without writing back) such that it is valid
/// for the given tensor shape.
///
/// This is a best effort function and will insert fallbacks as needed.
/// Note that fallbacks are defined on the spot here and don't use the component fallback system.
/// We don't need the fallback system here since we're also not using generic ui either.
///
/// General rules for scrubbing the input data:
/// * out of bounds dimensions and indices are clamped to valid
/// * missing width/height is filled in if there's at least 2 dimensions.
pub fn load_tensor_slice_selection_and_make_valid(
    slice_selection: &ViewProperty<'_>,
    shape: &[TensorDimension],
) -> Result<TensorSliceSelection, re_types::DeserializationError> {
    re_tracing::profile_function!();

    let max_valid_dim = shape.len().saturating_sub(1) as u32;

    let mut width = slice_selection.component_or_empty::<TensorWidthDimension>()?;
    let mut height = slice_selection.component_or_empty::<TensorHeightDimension>()?;

    // Clamp width and height to valid dimensions.
    if let Some(width) = width.as_mut() {
        width.dimension = width.dimension.at_most(max_valid_dim);
    }
    if let Some(height) = height.as_mut() {
        height.dimension = height.dimension.at_most(max_valid_dim);
    }

    // If there's more than two dimensions, force width and height to be set.
    if shape.len() >= 2 && (width.is_none() || height.is_none()) {
        let (default_width, default_height) = find_width_height_dim_indices(shape);
        if width.is_none() {
            width = Some(
                TensorDimensionSelection {
                    dimension: default_width as u32,
                    invert: shape[default_width]
                        .name
                        .as_ref()
                        .map_or(false, |name| name.to_lowercase().eq("left")),
                }
                .into(),
            );
        }
        if height.is_none() {
            height = Some(
                TensorDimensionSelection {
                    dimension: default_height as u32,
                    invert: shape[default_height]
                        .name
                        .as_ref()
                        .map_or(false, |name| name.to_lowercase().eq("up")),
                }
                .into(),
            );
        }
    }
    // If there's one dimension, force at least with or height to be set.
    else if shape.len() == 1 && width.is_none() && height.is_none() {
        width = Some(
            TensorDimensionSelection {
                dimension: 0,
                invert: false,
            }
            .into(),
        );
    }

    let width_dim = width.map_or(u32::MAX, |w| w.0.dimension);
    let height_dim = height.map_or(u32::MAX, |h| h.0.dimension);

    // -----

    let mut indices =
        slice_selection.component_array_or_empty::<TensorDimensionIndexSelection>()?;

    // Remove any index selection that uses a dimension that is out of bounds or equal to width/height.
    indices.retain(|index| {
        index.dimension < shape.len() as u32
            && index.dimension != width_dim
            && index.dimension != height_dim
    });

    // Clamp indices to valid dimension extent.
    let mut covered_dims = vec![false; shape.len()];
    for dim_index_selection in &mut indices {
        dim_index_selection.index = dim_index_selection
            .index
            .at_most(shape[dim_index_selection.dimension as usize].size - 1);
        covered_dims[dim_index_selection.dimension as usize] = true;
    }

    // Fill in missing indices for dimensions that aren't covered with the middle index.
    width.inspect(|w| covered_dims[w.dimension as usize] = true);
    height.inspect(|h| covered_dims[h.dimension as usize] = true);
    for (i, _) in covered_dims.into_iter().enumerate().filter(|(_, b)| !b) {
        indices.push(
            re_types::datatypes::TensorDimensionIndexSelection {
                dimension: i as u32,
                index: shape[i].size / 2,
            }
            .into(),
        );
    }

    // -----

    let slider = if let Some(mut slider) =
        slice_selection.component_array::<TensorDimensionIndexSlider>()?
    {
        // Remove any slider selection that uses a dimension that is out of bounds or equal to width/height.
        slider.retain(|slider| {
            slider.dimension < shape.len() as u32
                && slider.dimension != width_dim
                && slider.dimension != height_dim
        });
        slider
    } else {
        // If no slider were specified, create a default one for each dimension that isn't covered by width/height
        indices.iter().map(|index| index.dimension.into()).collect()
    };

    // -----

    Ok(TensorSliceSelection {
        width,
        height,
        indices: Some(indices),
        slider: Some(slider),
    })
}

#[allow(clippy::collapsible_else_if)]
fn find_width_height_dim_indices(shape: &[TensorDimension]) -> (usize, usize) {
    assert!(shape.len() >= 2);

    let mut width = None;
    let mut height = None;

    // First, go by name:
    for (i, dim) in shape.iter().enumerate() {
        let lowercase = dim
            .name
            .as_ref()
            .map(|name| name.to_lowercase())
            .unwrap_or_default();
        if is_name_like_width(&lowercase) {
            width = Some(i);
        }
        if is_name_like_height(&lowercase) {
            height = Some(i);
        }
    }

    if let (Some(width), Some(height)) = (width, height) {
        (width, height)
    } else {
        // Backup: go by length:
        let (longest, second_longest) = longest_and_second_longest_dim_indices(shape);

        if let Some(width) = width {
            let height = if width == longest {
                second_longest
            } else {
                longest
            };
            (width, height)
        } else if let Some(height) = height {
            let width = if height == longest {
                second_longest
            } else {
                longest
            };
            (width, height)
        } else {
            if (longest, second_longest) == (0, 1) || (longest, second_longest) == (1, 0) {
                // The first two dimensions - assume numpy ordering of [h, w, …]
                (1, 0)
            } else {
                (longest, second_longest)
            }
        }
    }
}

fn is_name_like_width(lowercase: &str) -> bool {
    matches!(lowercase, "w" | "width" | "right" | "left")
}

fn is_name_like_height(lowercase: &str) -> bool {
    matches!(lowercase, "h" | "height" | "up" | "down")
}

/// Returns the longest and second longest dimensions
fn longest_and_second_longest_dim_indices(shape: &[TensorDimension]) -> (usize, usize) {
    let mut longest_idx = 0;
    let mut second_longest_idx = 0;

    for (i, dim) in shape.iter().enumerate() {
        if dim.size > shape[longest_idx].size {
            second_longest_idx = longest_idx;
            longest_idx = i;
        } else if dim.size > shape[second_longest_idx].size {
            second_longest_idx = i;
        }
    }

    if longest_idx == second_longest_idx {
        (0, 1)
    } else {
        (longest_idx, second_longest_idx)
    }
}

#[test]
fn test_find_width_height_dim_indices() {
    fn named(size: u64, name: &str) -> TensorDimension {
        TensorDimension::named(size, name.to_owned())
    }

    fn dim(size: u64) -> TensorDimension {
        TensorDimension::unnamed(size)
    }
    let wh = find_width_height_dim_indices;

    assert_eq!(wh(&[dim(800), dim(50)]), (1, 0), "numpy ordering");
    assert_eq!(wh(&[dim(50), dim(800)]), (1, 0), "numpy ordering");
    assert_eq!(wh(&[dim(800), dim(50), dim(4)]), (1, 0), "numpy ordering");
    assert_eq!(wh(&[dim(50), dim(800), dim(4)]), (1, 0), "numpy ordering");
    assert_eq!(wh(&[dim(0), dim(0), dim(0)]), (1, 0), "numpy ordering");
    assert_eq!(wh(&[dim(10), dim(10), dim(10)]), (1, 0), "numpy ordering");

    assert_eq!(wh(&[dim(4), dim(50), dim(800)]), (2, 1), "longest=w");
    assert_eq!(
        wh(&[dim(4), dim(800), dim(50), dim(4)]),
        (1, 2),
        "longest=w"
    );

    assert_eq!(
        wh(&[named(2, "w"), named(3, "h"), dim(800)]),
        (0, 1),
        "fully named"
    );
    assert_eq!(
        wh(&[named(2, "height"), dim(800), named(3, "width")]),
        (2, 0),
        "fully named"
    );

    assert_eq!(
        wh(&[named(2, "w"), dim(50), dim(800)]),
        (0, 2),
        "partially named"
    );
    assert_eq!(
        wh(&[dim(50), dim(800), dim(10), named(20, "height")]),
        (1, 3),
        "partially named"
    );
}
