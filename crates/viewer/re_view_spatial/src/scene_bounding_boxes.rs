use egui::NumExt as _;
use nohash_hasher::IntMap;
use re_log_types::EntityPathHash;
use re_viewer_context::{SystemExecutionOutput, ViewClass as _};

use crate::view_kind::SpatialViewKind;
use crate::visualizers::iter_spatial_data;

#[derive(Clone)]
pub struct SceneBoundingBoxes {
    /// Overall bounding box of the scene for the current query.
    pub current: macaw::BoundingBox,

    /// Per-entity bounding boxes for the current query.
    pub per_entity: IntMap<EntityPathHash, macaw::BoundingBox>,

    /// Overall region of interest of the scene for the current query.
    ///
    /// For most entities this equals the bounding box, but may exclude outliers.
    /// Used for camera framing and other heuristics.
    pub region_of_interest_current: macaw::BoundingBox,

    /// A region of interest that smoothly transitions to the current one.
    ///
    /// If discontinuities are detected, this will be reset immediately.
    pub region_of_interest_smoothed: macaw::BoundingBox,

    /// Per-entity regions of interest for the current query.
    pub region_of_interest_per_entity: IntMap<EntityPathHash, macaw::BoundingBox>,
}

impl Default for SceneBoundingBoxes {
    fn default() -> Self {
        Self {
            current: macaw::BoundingBox::nothing(),
            per_entity: IntMap::default(),
            region_of_interest_current: macaw::BoundingBox::nothing(),
            region_of_interest_smoothed: macaw::BoundingBox::nothing(),
            region_of_interest_per_entity: IntMap::default(),
        }
    }
}

impl SceneBoundingBoxes {
    pub fn update(
        &mut self,
        ui: &egui::Ui,
        system_output: &SystemExecutionOutput,
        space_kind: SpatialViewKind,
    ) {
        re_tracing::profile_function!();

        let previous_region_of_interest = self.region_of_interest_current;
        self.current = macaw::BoundingBox::nothing();
        self.per_entity.clear();
        self.region_of_interest_current = macaw::BoundingBox::nothing();
        self.region_of_interest_per_entity.clear();

        for (affinity, data) in iter_spatial_data(system_output) {
            // If we're in a 3D space, but the visualizer is distinctly 2D, don't count it towards the bounding box.
            // These visualizers show up when we're on a pinhole camera plane which itself is heuristically fed by the
            // bounding box, creating a feedback loop if we were to add it here.
            if space_kind == SpatialViewKind::ThreeD
                && affinity == Some(crate::SpatialView2D::identifier())
            {
                continue;
            }

            for (entity, bbox) in data.iter_bounding_boxes() {
                self.per_entity
                    .entry(*entity)
                    .and_modify(|bbox_entry| *bbox_entry = bbox_entry.union(*bbox))
                    .or_insert(*bbox);
            }

            for (entity, region_of_interest) in data.iter_regions_of_interest() {
                self.region_of_interest_per_entity
                    .entry(*entity)
                    .and_modify(|entry| *entry = entry.union(*region_of_interest))
                    .or_insert(*region_of_interest);
            }
        }

        self.current = self
            .per_entity
            .values()
            .copied()
            .fold(macaw::BoundingBox::nothing(), macaw::BoundingBox::union);

        self.region_of_interest_current = self
            .region_of_interest_per_entity
            .values()
            .copied()
            .fold(macaw::BoundingBox::nothing(), macaw::BoundingBox::union);

        // Smooth the region of interest for stable camera behavior.
        let discontinuity =
            detect_discontinuity(self.region_of_interest_current, previous_region_of_interest);
        if !self.region_of_interest_smoothed.is_finite()
            || self.region_of_interest_smoothed.is_nothing()
            || discontinuity
        {
            self.region_of_interest_smoothed = self.region_of_interest_current;
        } else {
            let dt = ui.input(|input| input.stable_dt.at_most(0.1));

            let reach_this_factor = 0.9;
            let in_this_many_secs = 0.2;
            let smoothing_factor =
                egui::emath::exponential_smooth_factor(reach_this_factor, in_this_many_secs, dt);

            let current_center = self.region_of_interest_current.center();
            let current_size = self.region_of_interest_current.size();

            let new_smoothed_center = self
                .region_of_interest_smoothed
                .center()
                .lerp(current_center, smoothing_factor);
            let new_smoothed_size = self
                .region_of_interest_smoothed
                .size()
                .lerp(current_size, smoothing_factor);

            self.region_of_interest_smoothed =
                macaw::BoundingBox::from_center_size(new_smoothed_center, new_smoothed_size);

            let current_diagonal_length = current_size.length();
            let sameness_threshold = current_diagonal_length * (0.1 / 100.0); // 0.1% of the diagonal.
            if new_smoothed_center.distance(current_center) > sameness_threshold
                || (new_smoothed_size.length() - current_diagonal_length) / current_diagonal_length
                    > sameness_threshold
            {
                ui.request_repaint();
            }
        }
    }
}

fn detect_discontinuity(current: macaw::BoundingBox, previous: macaw::BoundingBox) -> bool {
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
