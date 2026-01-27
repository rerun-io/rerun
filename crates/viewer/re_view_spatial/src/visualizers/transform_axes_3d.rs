use re_entity_db::InstancePathHash;
use re_log_types::{EntityPath, Instance};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::{
    CoordinateFrame, InstancePoses3D, Pinhole, Transform3D, TransformAxes3D,
};
use re_sdk_types::components::{AxisLength, ShowLabels};
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, RequiredComponents, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use super::{SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget};
use crate::contexts::TransformTreeContext;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::utilities::format_transform_info_result;

pub struct TransformAxes3DVisualizer(SpatialViewVisualizerData);

impl Default for TransformAxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

impl IdentifiedViewSystem for TransformAxes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformAxes3D".into()
    }
}

impl VisualizerSystem for TransformAxes3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<TransformAxes3D>();

        // Make this visualizer available for any entity with Transform3D components
        query_info.required = RequiredComponents::AnyComponent(
            Transform3D::all_component_identifiers()
                .chain(CoordinateFrame::all_component_identifiers())
                .chain(InstancePoses3D::all_component_identifiers())
                .chain(Pinhole::all_component_identifiers())
                .chain(TransformAxes3D::all_component_identifiers())
                .collect(),
        );

        query_info
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();

        let transforms = context_systems.get::<TransformTreeContext>()?;

        let latest_at_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.latest_at);

        // Counting all transforms ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let entity_path = &data_result.entity_path;

            // Draw all transforms defined _at_ this entity.
            // TODO(RR-3319): consider also root frames here (not only child frames).
            let mut transforms_to_draw: smallvec::SmallVec<[_; 1]> = transforms
                .child_frames_for_entity(entity_path.hash())
                .map(|(frame_id_hash, transform)| {
                    let target_from_source = if let Some(target_from_camera) =
                        transforms.target_from_pinhole_root(*frame_id_hash)
                    {
                        // Don't apply the from-2D transform if this is a pinhole, stick with the last known 3D.
                        target_from_camera
                    } else {
                        transform.target_from_source
                    };

                    (*frame_id_hash, target_from_source.as_affine3a())
                })
                .collect();

            // We then *prepend* the axes for the entity's coordinate frame, because we want them to be drawn below
            // the additional transform data (the user usually knows which entity they are on).
            // TODO(grtlr): In the future we could make the `show_frame` component an enum to allow
            // for varying behavior.
            let coordinate_frame_transform_result =
                transforms.target_from_entity_path(entity_path.hash());

            match coordinate_frame_transform_result {
                Some(Ok(transform_info)) => {
                    let frame_id_hash = transforms.transform_frame_id_for(entity_path.hash());

                    if let Some(target_from_camera) =
                        transforms.target_from_pinhole_root(transform_info.tree_root())
                    {
                        // Don't apply the from-2D transform if this is a pinhole, stick with the last known 3D.
                        transforms_to_draw
                            .insert(0, (frame_id_hash, target_from_camera.as_affine3a()));
                    } else {
                        transforms_to_draw.insert_many(
                            0,
                            transform_info
                                .target_from_instances()
                                .iter()
                                .map(|t| (frame_id_hash, t.as_affine3a())),
                        );
                    }
                }

                // There are many reasons why a transform may be invalid and we want to report those.
                // However, if we already have named transforms to draw and the coordinate frame at this entity
                // is an implicit one, we skip reporting errors for it.
                Some(Err(re_tf::TransformFromToError::NoPathBetweenFrames { src, .. }))
                    if !transforms_to_draw.is_empty()
                        && src.as_entity_path_hash() == entity_path.hash() => {}

                _ => {
                    if let Err(err_msg) =
                        format_transform_info_result(transforms, coordinate_frame_transform_result)
                    {
                        output.report_error_for(entity_path.clone(), err_msg);
                    }
                }
            }

            // Early exit if there's nothing to do.
            if transforms_to_draw.is_empty() {
                return Ok(output);
            }

            let axis_length_identifier = TransformAxes3D::descriptor_axis_length().component;
            let show_frame_identifier = TransformAxes3D::descriptor_show_frame().component;

            // Note, we use this interface instead of `data_result.latest_at_with_blueprint_resolved_data` to avoid querying
            // for a bunch of unused components. The actual transform data comes out of the context manager and can't be
            // overridden via blueprint anyways.
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_at_query,
                data_result,
                [axis_length_identifier, show_frame_identifier],
                Some(instruction),
            );

            let axis_length: f32 = results
                .get_mono_with_fallback::<AxisLength>(axis_length_identifier)
                .into();

            if axis_length == 0.0 {
                // Don't draw axis and don't add to the bounding box!
                continue;
            }

            let show_frame: bool = results
                .get_mono_with_fallback::<ShowLabels>(show_frame_identifier)
                .into();

            // Draw axes for each instance
            for (instance_index, (label_id_hash, world_from_obj)) in
                transforms_to_draw.iter().enumerate()
            {
                if show_frame {
                    if let Some(frame_id) = transforms.lookup_frame_id(*label_id_hash) {
                        self.0.ui_labels.push(UiLabel {
                            text: frame_id.to_string(),
                            style: UiLabelStyle::Default,
                            target: UiLabelTarget::Position3D(
                                world_from_obj.transform_point3(glam::Vec3::ZERO),
                            ),
                            labeled_instance: InstancePathHash::entity_all(
                                &data_result.entity_path,
                            ),
                        });
                    } else {
                        // It should not be possible to hit this path and frame id hashes are not something that
                        // we should ever expose to our users, so let's add a debug assert for good measure.
                        debug_assert!(
                            false,
                            "[DEBUG ASSERT] unable to resolve frame id hash {label_id_hash:?}"
                        );
                        output.report_error_for(
                            data_result.entity_path.clone(),
                            format!("Could not resolve frame id hash {label_id_hash:?}"),
                        );
                    }
                }

                // Only add the center to the bounding box - the lines may be dependent on the bounding box, causing a feedback loop otherwise.
                self.0.add_bounding_box(
                    data_result.entity_path.hash(),
                    macaw::BoundingBox::ZERO,
                    *world_from_obj,
                );

                // Check for per-instance highlighting, fall back to overall entity highlighting
                let outline_mask = query
                    .highlights
                    .entity_outline_mask(data_result.entity_path.hash())
                    .instances
                    .get(&Instance::from(instance_index as u64))
                    .copied()
                    .unwrap_or_else(|| {
                        query
                            .highlights
                            .entity_outline_mask(data_result.entity_path.hash())
                            .overall
                    });

                add_axis_arrows(
                    ctx.tokens(),
                    &mut line_builder,
                    *world_from_obj,
                    Some(&data_result.entity_path),
                    axis_length,
                    outline_mask,
                    instance_index as u64,
                );
            }
        }

        Ok(output.with_draw_data([line_builder.into_draw_data()?.into()]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }
}

pub fn add_axis_arrows(
    tokens: &re_ui::DesignTokens,
    line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
    world_from_obj: glam::Affine3A,
    ent_path: Option<&EntityPath>,
    axis_length: f32,
    outline_mask_ids: re_renderer::OutlineMaskPreference,
    instance_index: u64,
) {
    use re_renderer::renderer::LineStripFlags;

    // TODO(andreas): It would be nice if could display the ViewCoordinates axis names (left/right/up) as a tooltip on hover.

    let line_radius = re_renderer::Size::new_ui_points(1.0);

    let mut line_batch = line_builder
        .batch(ent_path.map_or_else(|| "axis_arrows".to_owned(), |p| p.to_string()))
        .world_from_obj(world_from_obj)
        .triangle_cap_length_factor(10.0)
        .triangle_cap_width_factor(3.0)
        .outline_mask_ids(outline_mask_ids)
        .picking_object_id(re_renderer::PickingLayerObjectId(
            ent_path.map_or(0, |p| p.hash64()),
        ));
    let picking_instance_id = re_renderer::PickingLayerInstanceId(instance_index);

    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::X * axis_length)
        .radius(line_radius)
        .color(tokens.axis_color_x)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::Y * axis_length)
        .radius(line_radius)
        .color(tokens.axis_color_y)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::Z * axis_length)
        .radius(line_radius)
        .color(tokens.axis_color_z)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
}
