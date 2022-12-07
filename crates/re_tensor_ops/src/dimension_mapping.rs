#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
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
    pub fn create(num_dim: usize) -> DimensionMapping {
        // TODO(emilk): add a heuristic here for the default
        DimensionMapping {
            width: Some(1),
            height: Some(0),
            channel: None,
            selectors: (2..num_dim).collect(),
            invert_width: false,
            invert_height: false,
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
