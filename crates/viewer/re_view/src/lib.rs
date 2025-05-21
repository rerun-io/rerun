//! Rerun View utilities
//!
//! Types & utilities for defining View classes and communicating with the Viewport.

pub mod controls;

mod annotation_context_utils;
mod annotation_scene_context;
mod chunks_with_descriptor;
mod heuristics;
mod instance_hash_conversions;
mod outlines;
mod query;
mod results_ext;
mod view_property_ui;

pub use annotation_context_utils::{
    process_annotation_and_keypoint_slices, process_annotation_slices, process_color_slice,
};
pub use annotation_scene_context::AnnotationSceneContext;
pub use chunks_with_descriptor::{ChunkWithDescriptor, ChunksWithDescriptor};
pub use heuristics::suggest_view_for_each_entity;
pub use instance_hash_conversions::{
    instance_path_hash_from_picking_layer_id, picking_layer_id_from_instance_path_hash,
};
pub use outlines::{
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES, SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES, outline_config,
};
pub use query::{
    DataResultQuery, latest_at_with_blueprint_resolved_data, range_with_blueprint_resolved_data,
};
pub use results_ext::{
    HybridLatestAtResults, HybridRangeResults, HybridResults, HybridResultsChunkIter,
    RangeResultsExt,
};
pub use view_property_ui::{
    view_property_component_ui, view_property_component_ui_custom, view_property_ui,
};

pub mod external {
    pub use re_entity_db::external::*;
}

/// Clamp the last value in `values` in order to reach a length of `clamped_len`.
///
/// Returns an empty iterator if values is empty.
#[inline]
pub fn clamped_or_nothing<T>(values: &[T], clamped_len: usize) -> impl Iterator<Item = &T> + Clone {
    let Some(last) = values.last() else {
        return itertools::Either::Left(std::iter::empty());
    };

    itertools::Either::Right(
        values
            .iter()
            .chain(std::iter::repeat(last))
            .take(clamped_len),
    )
}
