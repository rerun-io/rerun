use egui::NumExt as _;
use nohash_hasher::IntMap;
use re_log_types::EntityPathHash;
use re_viewer_context::VisualizerCollection;

use crate::visualizers::SpatialViewVisualizerData;

#[derive(Clone)]
pub struct SceneBoundingBoxes {
    /// Accumulated bounding box over several frames.
    pub accumulated: macaw::BoundingBox,

    /// Overall bounding box of the scene for the current query.
    pub current: macaw::BoundingBox,

    /// A bounding box that smoothly transitions to the current bounding box.
    ///
    /// If discontinuities are detected, this bounding box will be reset immediately to the current bounding box.
    pub smoothed: macaw::BoundingBox,

    /// Per-entity bounding boxes for the current query.
    pub per_entity: IntMap<EntityPathHash, macaw::BoundingBox>,
}

impl Default for SceneBoundingBoxes {
    fn default() -> Self {
        Self {
            accumulated: macaw::BoundingBox::nothing(),
            current: macaw::BoundingBox::nothing(),
            smoothed: macaw::BoundingBox::nothing(),
            per_entity: IntMap::default(),
        }
    }
}

impl SceneBoundingBoxes {
    pub fn update(&mut self, ui: &egui::Ui, visualizers: &VisualizerCollection) {
        re_tracing::profile_function!();

        let previous = self.current;
        self.current = macaw::BoundingBox::nothing();
        self.per_entity.clear();

        for visualizer in visualizers.iter() {
            if let Some(data) = visualizer
                .data()
                .and_then(|d| d.downcast_ref::<SpatialViewVisualizerData>())
            {
                for (entity, bbox) in &data.bounding_boxes {
                    self.per_entity
                        .entry(*entity)
                        .and_modify(|bbox_entry| *bbox_entry = bbox_entry.union(*bbox))
                        .or_insert(*bbox);
                }
            }
        }

        for bbox in self.per_entity.values() {
            self.current = self.current.union(*bbox);
        }

        if self.accumulated.is_nothing() || !self.accumulated.size().is_finite() {
            self.accumulated = self.current;
        } else {
            self.accumulated = self.accumulated.union(self.current);
        }

        // Update smoothed bounding box.
        let discontinuity = detect_boundingbox_discontinuity(self.current, previous);
        if !self.smoothed.is_finite() || self.smoothed.is_nothing() || discontinuity {
            // Reset the smoothed bounding box if it's not valid or we detect a discontinuity.
            self.smoothed = self.current;
        } else {
            let dt = ui.input(|input| input.stable_dt.at_most(0.1));

            // Smooth the bounding box by moving center & size towards the current bounding box.
            let reach_this_factor = 0.9;
            let in_this_many_seconds = 0.2;
            let smoothing_factor =
                egui::emath::exponential_smooth_factor(reach_this_factor, in_this_many_seconds, dt);

            self.smoothed = macaw::BoundingBox::from_center_size(
                self.smoothed
                    .center()
                    .lerp(self.current.center(), smoothing_factor),
                self.smoothed
                    .size()
                    .lerp(self.current.size(), smoothing_factor),
            );

            if self.smoothed.min.distance_squared(self.current.min) > 0.001
                || self.smoothed.max.distance_squared(self.current.max) > 0.001
            {
                ui.ctx().request_repaint();
            }
        }
    }
}

fn detect_boundingbox_discontinuity(
    current: macaw::BoundingBox,
    previous: macaw::BoundingBox,
) -> bool {
    if !previous.is_finite() {
        // Previous bounding box is not finite, so we can't compare.
        return true;
    }

    // Is the size jumping a lot?
    let current_size = current.size().length();
    let previous_size = previous.size().length();
    let size_change = (current_size - previous_size).abs();
    let size_change_ratio = size_change / previous_size;
    if size_change_ratio > 0.5 {
        // Box size change by more than 50% since the previous frame.
        return true;
    }

    // Did the center jump?
    let current_center = current.center();
    let previous_center = previous.center();
    let center_change = current_center.distance(previous_center);
    let center_change_ratio = center_change / previous_size;
    if center_change_ratio > 0.5 {
        // Center change by more than 50% of the previous size since the previous frame.
        return true;
    }

    false
}
