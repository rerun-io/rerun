use re_log_types::{TimeInt, Timeline};
use re_view::{AnnotationSceneContext, DataResultQuery as _, HybridResults};
use re_types::Archetype;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewSystemExecutionError, ViewContext,
    ViewContextCollection, ViewQuery,
};

use crate::contexts::{EntityDepthOffsets, SpatialSceneEntityContext, TransformContext};

// ---

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
/// Returns an empty vctor if values is empty.
#[inline]
pub fn clamped_vec_or_empty<T: Clone>(values: &[T], clamped_len: usize) -> Vec<T> {
    if values.len() == clamped_len {
        // Happy path
        values.to_vec() // TODO(emilk): return a slice reference instead, in a `Cow` or similar
    } else if let Some(last) = values.last() {
        if values.len() == 1 {
            // Commo happy path
            return vec![last.clone(); clamped_len];
        } else if values.len() < clamped_len {
            // Clamp
            let mut vec = Vec::with_capacity(clamped_len);
            vec.extend(values.iter().cloned());
            vec.extend(std::iter::repeat(last.clone()).take(clamped_len - values.len()));
            vec
        } else {
            // Trim
            values.iter().take(clamped_len).cloned().collect()
        }
    } else {
        // Empty input
        Vec::new()
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

// --- Chunk-based APIs ---

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed along a [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
pub fn process_archetype<System: IdentifiedViewSystem, A, F>(
    ctx: &ViewContext<'_>,
    query: &ViewQuery<'_>,
    view_ctx: &ViewContextCollection,
    mut fun: F,
) -> Result<(), ViewSystemExecutionError>
where
    A: Archetype,
    F: FnMut(
        &QueryContext<'_>,
        &SpatialSceneEntityContext<'_>,
        &HybridResults<'_>,
    ) -> Result<(), ViewSystemExecutionError>,
{
    let transforms = view_ctx.get::<TransformContext>()?;
    let depth_offsets = view_ctx.get::<EntityDepthOffsets>()?;
    let annotations = view_ctx.get::<AnnotationSceneContext>()?;

    let latest_at = query.latest_at_query();

    let system_identifier = System::identifier();

    for data_result in query.iter_visible_data_results(ctx, system_identifier) {
        let Some(transform_info) = transforms.transform_info_for_entity(&data_result.entity_path)
        else {
            continue;
        };

        let depth_offset_key = (system_identifier, data_result.entity_path.hash());
        let entity_context = SpatialSceneEntityContext {
            transform_info,
            depth_offset: depth_offsets
                .per_entity_and_visualizer
                .get(&depth_offset_key)
                .copied()
                .unwrap_or_default(),
            annotations: annotations.0.find(&data_result.entity_path),
            highlight: query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash()),
            space_view_class_identifier: view_ctx.space_view_class_identifier(),
        };

        let results = data_result.query_archetype_with_history::<A>(ctx, query);

        let mut query_ctx = ctx.query_context(data_result, &latest_at);
        query_ctx.archetype_name = Some(A::name());

        {
            re_tracing::profile_scope!(format!("{}", data_result.entity_path));
            fun(&query_ctx, &entity_context, &results)?;
        }
    }

    Ok(())
}

// ---

use re_chunk::{Chunk, ChunkComponentIterItem, ComponentName, RowId};
use re_chunk_store::external::{re_chunk, re_chunk::external::arrow2};

/// Iterate `chunks` as indexed deserialized batches.
///
/// See [`Chunk::iter_component`] for more information.
#[allow(unused)]
pub fn iter_component<'a, C: re_types::Component>(
    chunks: &'a std::borrow::Cow<'a, [Chunk]>,
    timeline: Timeline,
    component_name: ComponentName,
) -> impl Iterator<Item = ((TimeInt, RowId), ChunkComponentIterItem<C>)> + 'a {
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(&timeline, &component_name),
            chunk.iter_component::<C>()
        )
    })
}

/// Iterate `chunks` as indexed primitives.
///
/// See [`Chunk::iter_primitive`] for more information.
#[allow(unused)]
pub fn iter_primitive<'a, T: arrow2::types::NativeType>(
    chunks: &'a std::borrow::Cow<'a, [Chunk]>,
    timeline: Timeline,
    component_name: ComponentName,
) -> impl Iterator<Item = ((TimeInt, RowId), &'a [T])> + 'a {
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(&timeline, &component_name),
            chunk.iter_primitive::<T>(&component_name)
        )
    })
}

/// Iterate `chunks` as indexed primitive arrays.
///
/// See [`Chunk::iter_primitive_array`] for more information.
#[allow(unused)]
pub fn iter_primitive_array<'a, const N: usize, T: arrow2::types::NativeType>(
    chunks: &'a std::borrow::Cow<'a, [Chunk]>,
    timeline: Timeline,
    component_name: ComponentName,
) -> impl Iterator<Item = ((TimeInt, RowId), &'a [[T; N]])> + 'a
where
    [T; N]: bytemuck::Pod,
{
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(&timeline, &component_name),
            chunk.iter_primitive_array::<N, T>(&component_name)
        )
    })
}

/// Iterate `chunks` as indexed list of primitive arrays.
///
/// See [`Chunk::iter_primitive_array_list`] for more information.
#[allow(unused)]
pub fn iter_primitive_array_list<'a, const N: usize, T: arrow2::types::NativeType>(
    chunks: &'a std::borrow::Cow<'a, [Chunk]>,
    timeline: Timeline,
    component_name: ComponentName,
) -> impl Iterator<Item = ((TimeInt, RowId), Vec<&'a [[T; N]]>)> + 'a
where
    [T; N]: bytemuck::Pod,
{
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(&timeline, &component_name),
            chunk.iter_primitive_array_list::<N, T>(&component_name)
        )
    })
}

/// Iterate `chunks` as indexed UTF-8 strings.
///
/// See [`Chunk::iter_string`] for more information.
#[allow(unused)]
pub fn iter_string<'a>(
    chunks: &'a std::borrow::Cow<'a, [Chunk]>,
    timeline: Timeline,
    component_name: ComponentName,
) -> impl Iterator<Item = ((TimeInt, RowId), Vec<re_types::ArrowString>)> + 'a {
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(&timeline, &component_name),
            chunk.iter_string(&component_name)
        )
    })
}

/// Iterate `chunks` as indexed buffers.
///
/// See [`Chunk::iter_buffer`] for more information.
#[allow(unused)]
pub fn iter_buffer<'a, T: arrow::datatypes::ArrowNativeType + arrow2::types::NativeType>(
    chunks: &'a std::borrow::Cow<'a, [Chunk]>,
    timeline: Timeline,
    component_name: ComponentName,
) -> impl Iterator<Item = ((TimeInt, RowId), Vec<re_types::ArrowBuffer<T>>)> + 'a {
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(&timeline, &component_name),
            chunk.iter_buffer(&component_name)
        )
    })
}
