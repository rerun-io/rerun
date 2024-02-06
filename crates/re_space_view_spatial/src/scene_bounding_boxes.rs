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

    /// Per-entity bounding boxes for the current query.
    pub per_entity: IntMap<EntityPathHash, macaw::BoundingBox>,
}

impl Default for SceneBoundingBoxes {
    fn default() -> Self {
        Self {
            accumulated: macaw::BoundingBox::nothing(),
            current: macaw::BoundingBox::nothing(),
            per_entity: IntMap::default(),
        }
    }
}

impl SceneBoundingBoxes {
    pub fn update(&mut self, visualizers: &VisualizerCollection) {
        re_tracing::profile_function!();

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
    }
}
