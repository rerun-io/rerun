use re_log_types::Tensor;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct DimensionMapping {
    /// Which dimensions have selectors?
    pub selectors: Vec<usize>,

    // Which dim?
    pub width: Option<usize>,

    // Which dim?
    pub height: Option<usize>,

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
        }
    }

    pub fn slice(
        &self,
        num_dim: usize,
        selector_values: &ahash::HashMap<usize, u64>,
    ) -> Vec<ndarray::SliceInfoElem> {
        (0..num_dim)
            .map(|dim| {
                if self.selectors.contains(&dim) {
                    ndarray::SliceInfoElem::Index(*selector_values.get(&dim).unwrap_or(&0) as _)
                } else {
                    ndarray::SliceInfoElem::Slice {
                        start: 0,
                        end: None,
                        step: 1,
                    }
                }
            })
            .collect()
    }
}
