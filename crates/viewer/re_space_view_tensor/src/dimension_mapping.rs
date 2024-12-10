use egui::NumExt as _;

use re_types::{
    blueprint::{archetypes::TensorSliceSelection, components::TensorDimensionIndexSlider},
    components::{TensorDimensionIndexSelection, TensorHeightDimension, TensorWidthDimension},
    datatypes::TensorDimensionSelection,
};
use re_viewport_blueprint::ViewProperty;

use crate::TensorDimension;

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
    slice_selection: &ViewProperty,
    shape: &[TensorDimension],
) -> Result<TensorSliceSelection, re_types::DeserializationError> {
    re_tracing::profile_function!();

    let mut width = slice_selection.component_or_empty::<TensorWidthDimension>()?;
    let mut height = slice_selection.component_or_empty::<TensorHeightDimension>()?;
    let mut indices =
        slice_selection.component_array_or_empty::<TensorDimensionIndexSelection>()?;
    let mut slider = slice_selection.component_array::<TensorDimensionIndexSlider>()?;

    make_width_height_valid(shape, &mut width, &mut height);
    make_indices_valid(shape, &mut indices, width, height);
    make_slider_valid(shape.len() as _, &mut slider, &indices, width, height);

    Ok(TensorSliceSelection {
        width,
        height,
        indices: Some(indices),
        slider,
    })
}

fn make_width_height_valid(
    shape: &[TensorDimension],
    width: &mut Option<TensorWidthDimension>,
    height: &mut Option<TensorHeightDimension>,
) {
    let max_valid_dim = shape.len().saturating_sub(1) as u32;

    // Clamp width and height to valid dimensions.
    if let Some(width) = width.as_mut() {
        width.dimension = width.dimension.at_most(max_valid_dim);
    }
    if let Some(height) = height.as_mut() {
        height.dimension = height.dimension.at_most(max_valid_dim);
    }

    // If height.dimension == width.dimension, remove height and go from there, pretending it was not set.
    if let (Some(some_width), Some(some_height)) = (&*width, &*height) {
        if some_width.dimension == some_height.dimension {
            height.take();
        }
    }

    // If there's more than two dimensions, force width and height to be set.
    if shape.len() >= 2 && (width.is_none() || height.is_none()) {
        let (default_width, default_height) = find_width_height_dim_indices(shape);
        if width.is_none() {
            *width = Some(
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
            *height = Some(
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
        *width = Some(
            TensorDimensionSelection {
                dimension: 0,
                invert: false,
            }
            .into(),
        );
    }
}

fn make_indices_valid(
    shape: &[TensorDimension],
    indices: &mut Vec<TensorDimensionIndexSelection>,
    width: Option<TensorWidthDimension>,
    height: Option<TensorHeightDimension>,
) {
    let width_dim = width.map_or(u32::MAX, |w| w.0.dimension);
    let height_dim = height.map_or(u32::MAX, |h| h.0.dimension);

    // Remove any index selection that uses a dimension that is out of bounds or equal to width/height.
    indices.retain(|index| {
        index.dimension < shape.len() as u32
            && index.dimension != width_dim
            && index.dimension != height_dim
    });

    // Clamp indices to valid dimension extent.
    let mut covered_dims = vec![false; shape.len()];
    for dim_index_selection in indices.iter_mut() {
        dim_index_selection.index = dim_index_selection.index.at_most(
            shape[dim_index_selection.dimension as usize]
                .size
                .saturating_sub(1),
        );
        covered_dims[dim_index_selection.dimension as usize] = true;
    }

    // Use middle index for dimensions that aren't covered.
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
}

fn make_slider_valid(
    num_dimensions: u32,
    slider: &mut Option<Vec<TensorDimensionIndexSlider>>,
    indices: &[TensorDimensionIndexSelection],
    width: Option<TensorWidthDimension>,
    height: Option<TensorHeightDimension>,
) {
    let width_dim = width.map_or(u32::MAX, |w| w.0.dimension);
    let height_dim = height.map_or(u32::MAX, |h| h.0.dimension);

    if let Some(slider) = slider.as_mut() {
        // Remove any slider selection that uses a dimension that is out of bounds or equal to width/height.
        slider.retain(|slider| {
            slider.dimension < num_dimensions
                && slider.dimension != width_dim
                && slider.dimension != height_dim
        });
    } else {
        // If no slider were specified, create a default one for each dimension that isn't covered by width/height
        *slider = Some(indices.iter().map(|index| index.dimension.into()).collect());
    };
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

#[cfg(test)]
mod tests {
    use crate::TensorDimension;
    use re_types::{
        blueprint::components::TensorDimensionIndexSlider,
        components::TensorDimensionIndexSelection,
    };

    use crate::dimension_mapping::{
        find_width_height_dim_indices, make_indices_valid, make_slider_valid,
        make_width_height_valid,
    };

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

    #[test]
    fn test_make_width_height_valid_multi_dim() {
        let shape = vec![
            TensorDimension::unnamed(100),
            TensorDimension::unnamed(100),
            TensorDimension::named(100, "width"),
            TensorDimension::unnamed(100),
        ];

        // Empty slice selection should produce a good default:
        let mut width = None;
        let mut height = None;
        make_width_height_valid(&shape, &mut width, &mut height);
        assert_eq!(width, Some(2.into()));
        assert_eq!(height, Some(0.into()));

        // If width/height are the same, new height is picked
        let mut width = Some(1.into());
        let mut height = None;
        make_width_height_valid(&shape, &mut width, &mut height);
        assert_eq!(width, Some(1.into()));
        assert_eq!(height, Some(0.into()));

        // Clamps width and height to valid dimensions, but doesn't pick the same.
        let mut width = Some(5.into());
        let mut height = Some(6.into());
        make_width_height_valid(&shape, &mut width, &mut height);
        assert_eq!(width, Some(3.into()));
        assert_eq!(height, Some(0.into()));

        // If both are valid, nothing happens.
        let mut width = Some(0.into());
        let mut height = Some(1.into());
        make_width_height_valid(&shape, &mut width, &mut height);
        assert_eq!(width, Some(0.into()));
        assert_eq!(height, Some(1.into()));
    }

    #[test]
    fn test_make_width_height_valid_single() {
        let shape = vec![TensorDimension::unnamed(100)];

        // Empty slice selection defaults width.
        let mut width = None;
        let mut height = None;
        make_width_height_valid(&shape, &mut width, &mut height);
        assert_eq!(width, Some(0.into()));
        assert_eq!(height, None);

        // If height is set, nothing happens.
        let mut width = None;
        let mut height = Some(0.into());
        make_width_height_valid(&shape, &mut width, &mut height);
        assert_eq!(width, None);
        assert_eq!(height, Some(0.into()));
    }

    #[test]
    fn test_make_indices_valid() {
        let shape = vec![
            TensorDimension::unnamed(100),
            TensorDimension::unnamed(200),
            TensorDimension::unnamed(300),
        ];

        // Invalid dimensions are removed.
        let mut indices = (0..10)
            .map(|i| TensorDimensionIndexSelection::new(i, 50))
            .collect();
        make_indices_valid(&shape, &mut indices, None, None);
        assert_eq!(
            indices,
            vec![
                TensorDimensionIndexSelection::new(0, 50),
                TensorDimensionIndexSelection::new(1, 50),
                TensorDimensionIndexSelection::new(2, 50)
            ],
        );

        // Invalid indices are clamped.
        let mut indices = (0..3)
            .map(|i| TensorDimensionIndexSelection::new(i, 1000))
            .collect();
        make_indices_valid(&shape, &mut indices, None, None);
        assert_eq!(
            indices,
            vec![
                TensorDimensionIndexSelection::new(0, 99),
                TensorDimensionIndexSelection::new(1, 199),
                TensorDimensionIndexSelection::new(2, 299)
            ],
        );

        // Indices that are covered by width/height are removed.
        let mut indices = (0..3)
            .map(|i| TensorDimensionIndexSelection::new(i, 0))
            .collect();
        make_indices_valid(&shape, &mut indices, Some(0.into()), Some(1.into()));
        assert_eq!(indices, vec![TensorDimensionIndexSelection::new(2, 0)],);
    }

    #[test]
    fn test_make_slider_valid() {
        let num_dim = 3;

        // Invalid dimensions are removed.
        let mut slider = Some((0..10).map(TensorDimensionIndexSlider::new).collect());
        make_slider_valid(num_dim, &mut slider, &[], None, None);
        assert_eq!(
            slider,
            Some(vec![
                TensorDimensionIndexSlider::new(0),
                TensorDimensionIndexSlider::new(1),
                TensorDimensionIndexSlider::new(2)
            ])
        );

        // Sliders for width/height are removed.
        let mut slider = Some((0..3).map(TensorDimensionIndexSlider::new).collect());
        make_slider_valid(num_dim, &mut slider, &[], Some(0.into()), Some(1.into()));
        assert_eq!(slider, Some(vec![TensorDimensionIndexSlider::new(2)]));

        // If no slider is set, default is created which is one for each index.
        let mut slider = None;
        let indices: Vec<_> = (0..2)
            .map(|i| TensorDimensionIndexSelection::new(i, 50))
            .collect();
        make_slider_valid(num_dim, &mut slider, &indices, None, None);
        assert_eq!(
            slider,
            Some(vec![
                TensorDimensionIndexSlider::new(0),
                TensorDimensionIndexSlider::new(1),
            ])
        );
    }
}
