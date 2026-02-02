//! Rerun View utilities
//!
//! Types & utilities for defining View classes and communicating with the Viewport.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

pub mod controls;

mod annotation_context_utils;
mod annotation_scene_context;
mod blueprint_resolved_results;
mod chunks_with_component;
mod instance_hash_conversions;
mod outlines;
mod query;
mod view_property_ui;
mod visualizer_query;

use std::borrow::Cow;

pub use annotation_context_utils::{
    process_annotation_and_keypoint_slices, process_annotation_slices, process_color_slice,
};
pub use annotation_scene_context::AnnotationSceneContext;
pub use blueprint_resolved_results::{
    BlueprintResolvedLatestAtResults, BlueprintResolvedRangeResults, BlueprintResolvedResults,
    BlueprintResolvedResultsExt, HybridResultsChunkIter,
};
pub use chunks_with_component::{
    ChunkWithComponent, ChunksWithComponent, MaybeChunksWithComponent,
};
pub use instance_hash_conversions::{
    instance_path_hash_from_picking_layer_id, picking_layer_id_from_instance_path_hash,
};
pub use outlines::{
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES, SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES, outline_config,
};
pub use query::{
    DataResultQuery, latest_at_with_blueprint_resolved_data, range_with_blueprint_resolved_data,
};
pub use view_property_ui::{
    view_property_component_ui, view_property_component_ui_custom, view_property_ui,
    view_property_ui_with_redirect,
};
pub use visualizer_query::VisualizerInstructionQueryResults;

pub mod external {
    pub use re_entity_db::external::*;
}

pub type ComponentMappingError = String;

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

/// Iterate over all the values in the slice, then repeat the last value forever.
///
/// If the input slice is empty, the second argument is returned forever.
#[inline]
pub fn clamped_or<'a, T>(values: &'a [T], if_empty: &'a T) -> impl Iterator<Item = &'a T> + Clone {
    let repeated = values.last().unwrap_or(if_empty);
    values.iter().chain(std::iter::repeat(repeated))
}

/// Clamp the last value in `values` in order to reach a length of `clamped_len`.
///
/// Returns an empty vector if values is empty.
#[inline]
pub fn clamped_vec_or_empty<T: Clone>(values: &[T], clamped_len: usize) -> Cow<'_, [T]> {
    if values.len() == clamped_len {
        // Happy path
        values.into()
    } else if let Some(last) = values.last() {
        if values.len() == 1 {
            // Commo happy path
            vec![last.clone(); clamped_len].into()
        } else if values.len() < clamped_len {
            // Clamp
            let mut vec = Vec::with_capacity(clamped_len);
            vec.extend(values.iter().cloned());
            vec.extend(std::iter::repeat_n(
                last.clone(),
                clamped_len - values.len(),
            ));
            vec.into()
        } else {
            // Trim
            values.iter().take(clamped_len).cloned().collect()
        }
    } else {
        // Empty input
        Vec::new().into()
    }
}

/// Clamp the last value in `values` in order to reach a length of `clamped_len`.
///
/// If the input slice is empty, the second argument is repeated `clamped_len` times.
#[inline]
pub fn clamped_vec_or<'a, T: Clone>(
    values: &'a [T],
    clamped_len: usize,
    if_empty: &T,
) -> Cow<'a, [T]> {
    let clamped = clamped_vec_or_empty(values, clamped_len);
    if clamped.is_empty() {
        vec![if_empty.clone(); clamped_len].into()
    } else {
        clamped
    }
}

/// Clamp the last value in `values` in order to reach a length of `clamped_len`.
///
/// If the input slice is empty, the second argument is repeated `clamped_len` times.
#[inline]
pub fn clamped_vec_or_else<T: Clone>(
    values: &[T],
    clamped_len: usize,
    if_empty: impl Fn() -> T,
) -> Cow<'_, [T]> {
    let clamped = clamped_vec_or_empty(values, clamped_len);
    if clamped.is_empty() {
        vec![if_empty(); clamped_len].into()
    } else {
        clamped
    }
}

#[test]
fn test_clamped_vec() {
    assert_eq!(clamped_vec_or_empty::<i32>(&[], 0), Vec::<i32>::default());
    assert_eq!(clamped_vec_or_empty::<i32>(&[], 3), Vec::<i32>::default());
    assert_eq!(
        clamped_vec_or_empty::<i32>(&[1, 2, 3], 0),
        Vec::<i32>::default()
    );
    assert_eq!(clamped_vec_or_empty::<i32>(&[1, 2, 3], 1), vec![1]);
    assert_eq!(clamped_vec_or_empty::<i32>(&[1, 2, 3], 2), vec![1, 2]);
    assert_eq!(clamped_vec_or_empty::<i32>(&[1, 2, 3], 3), vec![1, 2, 3]);
    assert_eq!(
        clamped_vec_or_empty::<i32>(&[1, 2, 3], 5),
        vec![1, 2, 3, 3, 3]
    );
}
