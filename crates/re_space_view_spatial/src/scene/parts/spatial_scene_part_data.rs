use crate::scene::UiLabel;

/// Common data struct for all spatial scene elements.
pub struct SpatialScenePartData {
    pub ui_labels: Vec<UiLabel>,
    pub bounding_box: macaw::BoundingBox,
}

impl Default for SpatialScenePartData {
    fn default() -> Self {
        Self {
            ui_labels: Vec::new(),
            bounding_box: macaw::BoundingBox::nothing(),
        }
    }
}
