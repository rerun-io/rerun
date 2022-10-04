use re_log_types::Tensor;

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
    pub fn create(tensor: &Tensor) -> DimensionMapping {
        // TODO(emilk): add a heuristic here for the default
        DimensionMapping {
            width: Some(1),
            height: Some(0),
            channel: None,
            selectors: (2..tensor.num_dim()).collect(),
            invert_width: false,
            invert_height: false,
        }
    }
}
