use egui::NumExt as _;
use nohash_hasher::IntMap;
use re_log_types::EntityPathHash;
use re_viewer_context::VisualizerCollection;

use crate::view_kind::SpatialViewKind;
use crate::visualizers::SpatialViewVisualizerData;

#[derive(Clone)]
pub struct SceneBoundingBoxes {
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
            current: macaw::BoundingBox::nothing(),
            smoothed: macaw::BoundingBox::nothing(),
            per_entity: IntMap::default(),
        }
    }
}

impl SceneBoundingBoxes {
    pub fn update(
        &mut self,
        ui: &egui::Ui,
        visualizers: &VisualizerCollection,
        space_kind: SpatialViewKind,
    ) {
        re_tracing::profile_function!();

        let previous = self.current;
        self.current = macaw::BoundingBox::nothing();
        self.per_entity.clear();

        for data in visualizers.iter_visualizer_data::<SpatialViewVisualizerData>() {
            // If we're in a 3D space, but the visualizer is distintivly 2D, don't count it towards the bounding box.
            // These visualizers show up when we're on a pinhole camera plane which itself is heuristically fed by the
            // bounding box, creating a feedback loop if we were to add it here.
            let data_is_only_2d = data
                .preferred_view_kind
                .is_some_and(|kind| kind == SpatialViewKind::TwoD);
            if space_kind == SpatialViewKind::ThreeD && data_is_only_2d {
                continue;
            }

            for (entity, bbox) in &data.bounding_boxes {
                self.per_entity
                    .entry(*entity)
                    .and_modify(|bbox_entry| *bbox_entry = bbox_entry.union(*bbox))
                    .or_insert(*bbox);
            }
        }

        #[expect(clippy::iter_over_hash_type)] // order-independent:
        for bbox in self.per_entity.values() {
            self.current = self.current.union(*bbox);
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
            let in_this_many_secs = 0.2;
            let smoothing_factor =
                egui::emath::exponential_smooth_factor(reach_this_factor, in_this_many_secs, dt);

            let current_center = self.current.center();
            let current_size = self.current.size();

            let new_smoothed_center = self
                .smoothed
                .center()
                .lerp(current_center, smoothing_factor);
            let new_smoothed_size = self.smoothed.size().lerp(current_size, smoothing_factor);

            self.smoothed =
                macaw::BoundingBox::from_center_size(new_smoothed_center, new_smoothed_size);

            let current_diagonal_length = current_size.length();
            let sameness_threshold = current_diagonal_length * (0.1 / 100.0); // 0.1% of the diagonal.
            if new_smoothed_center.distance(current_center) > sameness_threshold
                || (new_smoothed_size.length() - current_diagonal_length) / current_diagonal_length
                    > sameness_threshold
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
