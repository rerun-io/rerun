#![allow(clippy::collapsible_else_if)]

use re_log_types::TensorDimension;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DimensionMapping {
    /// Which dimensions have selectors?
    pub selectors: Vec<usize>,

    // Which dim?
    pub width: Option<usize>,

    // Which dim?
    pub height: Option<usize>,

    /// Flip the width
    pub invert_width: bool,

    /// Flip the height
    pub invert_height: bool,

    // Which dim?
    pub channel: Option<usize>,
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
                channel: None,
            },

            1 => DimensionMapping {
                selectors: vec![0],
                width: None,
                height: None,
                invert_width: false,
                invert_height: false,
                channel: None,
            },

            _ => {
                let (width, height) = find_width_height(shape);
                let selectors = (0..shape.len())
                    .filter(|&i| i != width && i != height)
                    .collect();

                DimensionMapping {
                    selectors,
                    width: Some(width),
                    height: Some(height),
                    invert_width: shape[width].name.to_lowercase() == "left",
                    invert_height: shape[height].name.to_lowercase() == "up",
                    channel: None,
                }
            }
        }
    }

    /// Protect against old serialized data that is not up-to-date with the new tensor
    pub fn is_valid(&self, num_dim: usize) -> bool {
        fn is_valid(dim_selector: &Option<usize>, num_dim: usize) -> bool {
            if let Some(dim) = dim_selector {
                *dim < num_dim
            } else {
                true
            }
        }

        is_valid(&self.width, num_dim)
            && is_valid(&self.height, num_dim)
            && is_valid(&self.channel, num_dim)
    }
}

fn find_width_height(shape: &[TensorDimension]) -> (usize, usize) {
    assert!(shape.len() >= 2);

    let mut width = None;
    let mut height = None;

    // First, go by name:
    for (i, dim) in shape.iter().enumerate() {
        let lowercase = dim.name.to_lowercase();
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
        let (longest, second_longest) = longest_and_second_longest(shape);

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
fn longest_and_second_longest(shape: &[TensorDimension]) -> (usize, usize) {
    let mut longest = 0;
    let mut second_longest = 0;

    for (i, dim) in shape.iter().enumerate() {
        if dim.size > shape[longest].size {
            second_longest = longest;
            longest = i;
        } else if dim.size > shape[second_longest].size {
            second_longest = i;
        }
    }

    if longest == second_longest {
        // A shape of all-zeros
        debug_assert!(longest == 0);
        (0, 1)
    } else {
        (longest, second_longest)
    }
}

#[test]
fn test_auto_dim_mapping() {
    fn named(size: u64, name: &str) -> TensorDimension {
        TensorDimension::named(size, name.to_owned())
    }
    fn dim(size: u64) -> TensorDimension {
        TensorDimension::unnamed(size)
    }
    let wh = find_width_height;

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
