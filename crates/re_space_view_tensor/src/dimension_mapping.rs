use re_types::datatypes::TensorDimension;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct DimensionSelector {
    pub visible: bool,
    pub dim_idx: usize,
}

impl DimensionSelector {
    pub fn new(dim_idx: usize) -> Self {
        DimensionSelector {
            visible: true,
            dim_idx,
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct DimensionMapping {
    /// Which dimensions have selectors, and are they visible?
    pub selectors: Vec<DimensionSelector>,

    // Which dim?
    pub width: Option<usize>,

    // Which dim?
    pub height: Option<usize>,

    /// Flip the width
    pub invert_width: bool,

    /// Flip the height
    pub invert_height: bool,
}

impl DimensionMapping {
    pub fn create(shape: &[TensorDimension]) -> DimensionMapping {
        match shape.len() {
            0 => DimensionMapping {
                selectors: Default::default(),
                width: None,
                height: None,
                invert_width: false,
                invert_height: false,
            },

            1 => DimensionMapping {
                selectors: vec![DimensionSelector::new(0)],
                width: None,
                height: None,
                invert_width: false,
                invert_height: false,
            },

            _ => {
                let (width, height) = find_width_height_dim_indices(shape);
                let selectors = (0..shape.len())
                    .filter(|i| *i != width && *i != height)
                    .map(DimensionSelector::new)
                    .collect();

                let invert_width = shape[width]
                    .name
                    .as_ref()
                    .map(|name| name.to_lowercase().eq("left"))
                    .unwrap_or_default();
                let invert_height = shape[height]
                    .name
                    .as_ref()
                    .map(|name| name.to_lowercase().eq("up"))
                    .unwrap_or_default();

                DimensionMapping {
                    selectors,
                    width: Some(width),
                    height: Some(height),
                    invert_width,
                    invert_height,
                }
            }
        }
    }

    /// Protect against old serialized data that is not up-to-date with the new tensor
    pub fn is_valid(&self, num_dim: usize) -> bool {
        fn is_in_range(dim_selector: &Option<usize>, num_dim: usize) -> bool {
            if let Some(dim) = dim_selector {
                *dim < num_dim
            } else {
                true
            }
        }

        let mut used_dimensions: ahash::HashSet<usize> =
            self.selectors.iter().map(|s| s.dim_idx).collect();
        if let Some(width) = self.width {
            used_dimensions.insert(width);
        }
        if let Some(height) = self.height {
            used_dimensions.insert(height);
        }
        if used_dimensions.len() != num_dim {
            return false;
        }

        // we should have both width and height set…
        (num_dim < 2 || (self.width.is_some() && self.height.is_some()))

        // …and all dimensions should be in range
            && is_in_range(&self.width, num_dim)
            && is_in_range(&self.height, num_dim)
    }
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
