//! Materializes visualizer components whose selected source is annotation context.
//!
//! Annotation context does not directly contain per-instance component batches.
//! It maps class IDs, and optionally keypoint IDs, to metadata such as colors and labels.
//! This module combines that metadata with the queried ID components to produce ordinary component chunks consumed by visualizers.
//!
//! Resolution first merges the independently updated ID components into one ordered event stream that is reused for every annotation-derived target.
//! Each target then walks that stream, keeping the latest class and keypoint IDs active while emitting resolved component chunks.
//!
//! Latest-at resolution retains only the final emitted row as a static unit chunk, while range resolution collects every emitted row on the query timeline.
//! Only targets explicitly mapped to [`ComponentSourceKind::AnnotationContext`] are materialized.

use itertools::Either;

use re_chunk_store::external::re_chunk::external::arrow::array::ArrayRef;
use re_chunk_store::external::re_chunk::{self, RowId, TimeInt, TimelineName};
use re_sdk_types::FromArrow as _;
use re_sdk_types::blueprint::encodings::ComponentSourceKind;
use re_sdk_types::components::{ClassId, Color, KeypointId, Text};
use re_viewer_context::{AnnotationContextTarget, AnnotationContextTargetKind, typed_fallback_for};

use crate::blueprint_resolved_results::ComponentSourcesMap;
use crate::{
    BlueprintResolvedLatestAtResults, BlueprintResolvedRangeResults, BlueprintResolvedResultsExt,
    ChunksWithComponent, ComponentMappingError, MaybeChunksWithComponent,
};

/// One update to the state used for annotation-context resolution.
///
/// Components can be logged independently, so ordering by `(time, row_id)` provides a deterministic stream in which values remain active until superseded.
struct AnnotationComponentEvent {
    index: (TimeInt, RowId),
    timepoint: re_log_types::TimePoint,
    data: AnnotationComponentEventData,
}

/// The inputs needed to turn annotation metadata into per-instance component values.
enum AnnotationComponentEventData {
    /// Selects a class description for each instance.
    ClassIds(re_chunk::ChunkComponentIterItem<ClassId>),

    /// Selects keypoint-specific metadata within each instance's class description.
    KeypointIds(re_chunk::ChunkComponentIterItem<KeypointId>),
}

/// Extracts typed component updates from resolved query chunks.
fn component_events<C: re_types_core::Component>(
    chunks: MaybeChunksWithComponent<'_>,
    timeline: TimelineName,
    wrap: impl Fn(re_chunk::ChunkComponentIterItem<C>) -> AnnotationComponentEventData,
) -> Result<Vec<AnnotationComponentEvent>, ComponentMappingError> {
    let chunks = ChunksWithComponent::try_from(chunks).map_err(ComponentMappingError::clone)?;
    let mut events = Vec::new();

    for chunk in chunks.iter() {
        events.extend(
            itertools::izip!(
                chunk.iter_component_indices(timeline),
                chunk.iter_component_timepoints(),
                chunk.iter_component::<C>(),
            )
            .map(|(index, timepoint, values)| AnnotationComponentEvent {
                index,
                timepoint,
                data: wrap(values),
            }),
        );
    }

    Ok(events)
}

/// Builds the ordered input stream shared by all annotation-context targets in a query.
///
/// Component mapping failures are returned so every target depending on this stream can report the same source error.
fn annotation_component_events<'a>(
    results: &'a impl BlueprintResolvedResultsExt<'a>,
    annotation_query: &re_viewer_context::AnnotationContextQuery,
    timeline: TimelineName,
) -> Result<Vec<AnnotationComponentEvent>, ComponentMappingError> {
    let mut events = component_events::<ClassId>(
        results.get_optional_chunks(annotation_query.class_ids),
        timeline,
        AnnotationComponentEventData::ClassIds,
    )?;
    if let Some(component) = annotation_query.keypoint_ids {
        events.extend(component_events::<KeypointId>(
            results.get_optional_chunks(component),
            timeline,
            AnnotationComponentEventData::KeypointIds,
        )?);
    }
    events.sort_by_key(|event| event.index);

    Ok(events)
}

/// Materializes one target component from independently logged ID batches.
///
/// Missing annotation metadata is filled by `default_for_instance`.
fn resolve_annotation_target_typed<'a, C: re_types_core::Component + Clone>(
    entity_path: &'a re_log_types::EntityPath,
    target: &'a AnnotationContextTarget,
    events: &'a [AnnotationComponentEvent],
    annotations: &'a re_viewer_context::Annotations,
    default_for_instance: impl Fn(usize) -> C + 'a,
    annotation_value: impl Fn(&re_viewer_context::ResolvedAnnotationInfo) -> Option<C> + 'a,
) -> impl Iterator<Item = re_chunk::Chunk> + 'a {
    let mut class_ids: &'a [ClassId] = &[];
    let mut keypoint_ids: &'a [KeypointId] = &[];

    let mut timepoint = None;
    events
        .iter()
        .enumerate()
        .filter_map(move |(event_index, event)| {
            timepoint.get_or_insert_with(|| event.timepoint.clone());
            match &event.data {
                AnnotationComponentEventData::ClassIds(values) => class_ids = values,
                AnnotationComponentEventData::KeypointIds(values) => keypoint_ids = values,
            }
            if events
                .get(event_index + 1)
                .is_some_and(|next_event| next_event.index == event.index)
            {
                return None;
            }

            let last_class_id;
            let values = if let Some(last_keypoint_id) = keypoint_ids.last() {
                let num_elements = std::cmp::max(class_ids.len(), keypoint_ids.len());
                last_class_id = class_ids
                    .last()
                    .copied()
                    .unwrap_or_else(|| ClassId::from(0));

                Either::Right(
                    std::iter::zip(
                        std::iter::chain(class_ids.iter(), std::iter::repeat(&last_class_id)),
                        std::iter::chain(keypoint_ids.iter(), std::iter::repeat(last_keypoint_id)),
                    )
                    .take(num_elements)
                    .map(|(class_id, keypoint_id)| {
                        let class = annotations.resolved_class_description(Some(*class_id));
                        annotation_value(&class.annotation_info_with_keypoint(**keypoint_id))
                    }),
                )
            } else {
                Either::Left(class_ids.iter().map(|class_id| {
                    let class = annotations.resolved_class_description(Some(*class_id));
                    annotation_value(&class.annotation_info())
                }))
            };

            // TODO(andreas): `to_arrow` is kinda costly, at least for colors also rather unnecessary, we could just do a cast.
            let array = C::to_arrow(values.enumerate().map(|(i, value)| {
                std::borrow::Cow::Owned(value.unwrap_or_else(|| default_for_instance(i)))
            }))
            .ok()?;
            re_chunk::Chunk::builder(entity_path.clone())
                .with_row(
                    event.index.1,
                    timepoint.take()?,
                    [(target.descriptor().clone(), array)],
                )
                .build()
                .ok()
        })
}

/// Selects the per-instance view default using the same last-value splatting as component batches, then falls back to the visualizer default.
fn default_for_instance<C: Clone>(
    default_values: Option<&[C]>,
    fallback: &C,
    instance: usize,
) -> C {
    default_values
        .and_then(|values| values.get(instance).or_else(|| values.last()))
        .cloned()
        .unwrap_or_else(|| fallback.clone())
}

/// Dispatches target-specific annotation lookup and fallback behavior.
///
/// Colors use authored annotation colors when available, otherwise a deterministic color synthesized from the annotation or class ID.
/// Labels discard empty annotation strings so view or visualizer defaults can fill them.
fn resolve_annotation_target<'a>(
    query_context: &re_viewer_context::QueryContext<'_>,
    entity_path: &'a re_log_types::EntityPath,
    target: &'a AnnotationContextTarget,
    events: &'a [AnnotationComponentEvent],
    annotations: &'a re_viewer_context::Annotations,
    default_batch: Option<&ArrayRef>,
) -> impl Iterator<Item = re_chunk::Chunk> + 'a {
    match target.kind() {
        AnnotationContextTargetKind::Color => {
            let default_values = default_batch.and_then(|batch| Color::from_arrow(batch).ok());
            let fallback = typed_fallback_for(query_context, target.descriptor().component);
            Either::Left(resolve_annotation_target_typed(
                entity_path,
                target,
                events,
                annotations,
                move |instance| {
                    default_for_instance(default_values.as_deref(), &fallback, instance)
                },
                // Note that `annotation.color` applies a kind of `class_id` based fallback.
                // This is intentional - we essentially regard all classes to always have a color, even if not explicitly authored.
                |annotation| annotation.color().map(|color| Color(color.into())),
            ))
        }

        AnnotationContextTargetKind::Label => {
            let default_values = default_batch.and_then(|batch| Text::from_arrow(batch).ok());
            let fallback = typed_fallback_for(query_context, target.descriptor().component);
            Either::Right(resolve_annotation_target_typed(
                entity_path,
                target,
                events,
                annotations,
                move |instance| {
                    default_for_instance(default_values.as_deref(), &fallback, instance)
                },
                |annotation| {
                    annotation
                        .annotation_info
                        .as_ref()
                        .and_then(|info| info.label.clone())
                        .filter(|label| !label.as_str().is_empty())
                        .map(Text)
                },
            ))
        }
    }
}

/// Returns valid targets whose checked component source selected annotation context.
///
/// Targets with an earlier source-selection error are skipped so that error remains authoritative.
fn annotation_targets_to_resolve<'a>(
    annotation_query: &'a re_viewer_context::AnnotationContextQuery,
    component_sources: &ComponentSourcesMap<'_>,
) -> Vec<&'a AnnotationContextTarget> {
    let annotation_targets = annotation_query.targets.iter();
    annotation_targets
        .filter(|target| {
            matches!(
                component_sources.get(&target.descriptor().component),
                Some(source) if source.error().is_none() && source.source().source_kind() == ComponentSourceKind::AnnotationContext
            )
        })
        .collect()
}

impl BlueprintResolvedLatestAtResults<'_> {
    /// Materializes the final annotation-derived row for every selected target.
    ///
    /// The row is normalized to a static, zero-indexed unit chunk to match other latest-at blueprint-resolved sources.
    pub fn resolve_annotation_context(
        &mut self,
        annotations: Option<&re_viewer_context::Annotations>,
        annotation_query: Option<&re_viewer_context::AnnotationContextQuery>,
    ) {
        let Some(annotation_query) = annotation_query else {
            return;
        };
        // We may still generate colors from class_ids even if there's no annotation context available.
        let annotations =
            annotations.unwrap_or_else(|| re_viewer_context::Annotations::missing_ref());

        let timeline = self
            .query_context
            .query
            .timeline()
            // Static chunks ignore the timeline, but component event iteration requires one.
            .unwrap_or_else(TimelineName::log_tick);
        let targets = annotation_targets_to_resolve(annotation_query, &self.component_sources);
        if targets.is_empty() {
            return;
        }

        // For latest-at there should be just a single event (or two for classids + keypoints).
        // The event logic is mostly important for range queries, but we go through it here as well to share some code.
        let events = annotation_component_events(self, annotation_query, timeline);

        for target in targets {
            let component = target.descriptor().component;
            let last_chunk = match &events {
                Ok(events) => resolve_annotation_target(
                    &self.query_context,
                    self.query_context.target_entity_path,
                    target,
                    events,
                    annotations,
                    self.view_defaults
                        .get(component)
                        .and_then(|chunk| chunk.component_batch_raw(component))
                        .as_ref(),
                )
                .last(),
                Err(err) => {
                    if let Some(source) = self.component_sources.get_mut(&component) {
                        source.set_error((*err).clone());
                    }
                    continue;
                }
            };

            if let Some(chunk) = last_chunk {
                let Some(unit_chunk) = chunk.into_static().zeroed().into_unit() else {
                    if let Some(source) = self.component_sources.get_mut(&component) {
                        source.set_error(ComponentMappingError::AnnotationContextUnavailable(
                            component,
                        ));
                    }
                    continue;
                };
                self.annotation_resolved.insert(component, unit_chunk);
            }
        }
    }
}

impl BlueprintResolvedRangeResults<'_> {
    /// Materializes every annotation-derived row for selected targets over `timeline`.
    pub fn resolve_annotation_context(
        &mut self,
        annotations: Option<&re_viewer_context::Annotations>,
        annotation_query: Option<&re_viewer_context::AnnotationContextQuery>,
        timeline: TimelineName,
    ) {
        let Some(annotation_query) = annotation_query else {
            return;
        };
        // We may still generate colors from class_ids even if there's no annotation context available.
        let annotations =
            annotations.unwrap_or_else(|| re_viewer_context::Annotations::missing_ref());
        let targets = annotation_targets_to_resolve(annotation_query, &self.component_sources);
        if targets.is_empty() {
            return;
        }
        let events = annotation_component_events(self, annotation_query, timeline);

        for target in targets {
            let component = target.descriptor().component;
            let default_batch = self
                .view_defaults
                .get(component)
                .and_then(|chunk| chunk.component_batch_raw(component));

            // Since we're synthesizing chunks from individual events we end up with one chunk per row.
            let rows = match &events {
                Ok(events) => resolve_annotation_target(
                    &self.query_context,
                    self.query_context.target_entity_path,
                    target,
                    events,
                    annotations,
                    default_batch.as_ref(),
                )
                .collect(),
                Err(err) => {
                    if let Some(source) = self.component_sources.get_mut(&component) {
                        source.set_error((*err).clone());
                    }
                    continue;
                }
            };

            self.annotation_resolved.insert(component, rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::{EntityPath, TimePoint, Timeline};
    use re_sdk_types::datatypes::Rgba32;
    use re_types_core::{Component as _, ComponentDescriptor};

    use super::*;

    fn component_batch<C: re_types_core::Component + Clone>(
        values: impl IntoIterator<Item = C>,
    ) -> re_chunk::ChunkComponentIterItem<C> {
        let descriptor = ComponentDescriptor::partial("test.input").with_component_type(C::name());
        let array = C::to_arrow(
            values
                .into_iter()
                .map(|value| std::borrow::Cow::Owned(value)),
        )
        .unwrap();
        let chunk = re_chunk::Chunk::builder(EntityPath::from("entity"))
            .with_row(
                RowId::new(),
                TimePoint::STATIC,
                [(descriptor.clone(), array)],
            )
            .build()
            .unwrap();
        chunk
            .iter_component::<C>(descriptor.component)
            .next()
            .unwrap()
    }

    /// Class and keypoint ID batches are independently splatted to the longest active batch.
    /// An explicit empty class-ID batch resets the classes while retained keypoint IDs resolve against class zero.
    #[test]
    fn resolves_multiple_class_ids_with_keypoints_using_clamping() {
        let target = AnnotationContextTarget::color(
            ComponentDescriptor::partial("test.colors").with_component_type(Color::name()),
        );
        let make_event = |time, row_id, data| AnnotationComponentEvent {
            index: (TimeInt::new_temporal(time), row_id),
            timepoint: TimePoint::from_iter([(Timeline::new_sequence("frame"), time)]),
            data,
        };
        let row_ids = [RowId::new(), RowId::new(), RowId::new()];
        let events = vec![
            make_event(
                1,
                row_ids[0],
                AnnotationComponentEventData::ClassIds(component_batch([
                    ClassId::from(1),
                    ClassId::from(2),
                ])),
            ),
            make_event(
                1,
                row_ids[0],
                AnnotationComponentEventData::KeypointIds(component_batch([KeypointId::from(10)])),
            ),
            make_event(
                2,
                row_ids[1],
                AnnotationComponentEventData::ClassIds(component_batch([ClassId::from(3)])),
            ),
            make_event(
                2,
                row_ids[1],
                AnnotationComponentEventData::KeypointIds(component_batch([
                    KeypointId::from(10),
                    KeypointId::from(20),
                ])),
            ),
            make_event(
                3,
                row_ids[2],
                AnnotationComponentEventData::ClassIds(component_batch([])),
            ),
        ];

        let resolved = resolve_annotation_target_typed(
            &EntityPath::from("entity"),
            &target,
            &events,
            &re_viewer_context::Annotations::missing(),
            |_| Color(Rgba32::WHITE),
            |annotation| annotation.color().map(|color| Color(color.into())),
        )
        .collect::<Vec<_>>();
        let resolved_colors = resolved
            .iter()
            .map(|chunk| {
                chunk
                    .iter_component::<Color>(target.descriptor().component)
                    .next()
                    .unwrap()
                    .as_slice()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let class_color = |class_id| {
            Color(
                re_viewer_context::ResolvedAnnotationInfo {
                    class_id: Some(*ClassId::from(class_id)),
                    annotation_info: None,
                }
                .color()
                .unwrap()
                .into(),
            )
        };

        assert_eq!(
            resolved_colors,
            [
                vec![class_color(1), class_color(2)],
                vec![class_color(3), class_color(3)],
                vec![class_color(0), class_color(0)],
            ]
        );
    }
}
