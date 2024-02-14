use re_entity_db::EntityPath;
use re_renderer::LineBatchesBuilder;
use re_types::{
    archetypes::Boxes3D,
    components::{
        ClassId, Color, HalfSizes3D, InstanceKey, KeypointId, Position3D, Radius, Rotation3D, Text,
    },
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{UiLabel, UiLabelTarget},
};

use super::{
    filter_visualizable_3d_entities, picking_id_from_instance_key,
    process_annotation_and_keypoint_slices, process_color_slice, process_label_slice,
    process_radius_slice, SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

pub struct Boxes3DVisualizer(SpatialViewVisualizerData);

impl Default for Boxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

impl Boxes3DVisualizer {
    fn process_data(
        &mut self,
        line_builder: &mut LineBatchesBuilder,
        query: &ViewQuery<'_>,
        data: &Boxes3DComponentData<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) {
        let (annotation_infos, _) = process_annotation_and_keypoint_slices(
            query.latest_at,
            data.instance_keys,
            data.keypoint_ids,
            data.class_ids,
            data.half_sizes.iter().map(|_| glam::Vec3::ZERO),
            &ent_context.annotations,
        );

        let centers = || {
            data.centers
                .as_ref()
                .map_or(
                    itertools::Either::Left(std::iter::repeat(&None).take(data.half_sizes.len())),
                    |data| itertools::Either::Right(data.iter()),
                )
                .map(|center| center.unwrap_or(Position3D::ZERO))
        };
        let rotations = || {
            data.rotations
                .as_ref()
                .map_or(
                    itertools::Either::Left(std::iter::repeat(&None).take(data.half_sizes.len())),
                    |data| itertools::Either::Right(data.iter()),
                )
                .map(|center| center.clone().unwrap_or(Rotation3D::IDENTITY))
        };

        let radii = process_radius_slice(data.radii, data.half_sizes.len(), ent_path);
        let colors = process_color_slice(data.colors, ent_path, &annotation_infos);
        let labels = process_label_slice(data.labels, data.half_sizes.len(), &annotation_infos);

        let mut line_batch = line_builder
            .batch("boxes3d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let mut bounding_box = macaw::BoundingBox::nothing();

        for (instance_key, half_size, center, rotation, radius, color, label) in itertools::izip!(
            data.instance_keys,
            data.half_sizes,
            centers(),
            rotations(),
            radii,
            colors,
            labels,
        ) {
            let instance_hash = re_entity_db::InstancePathHash::instance(ent_path, *instance_key);

            bounding_box.extend(half_size.box_min(center));
            bounding_box.extend(half_size.box_max(center));

            let center = center.into();

            let box3d = line_batch
                .add_box_outline_from_transform(glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::from(*half_size) * 2.0,
                    rotation.into(),
                    center,
                ))
                .color(color)
                .radius(radius)
                .picking_instance_id(picking_id_from_instance_key(*instance_key));
            if let Some(outline_mask_ids) = ent_context
                .highlight
                .instances
                .get(&instance_hash.instance_key)
            {
                box3d.outline_mask_ids(*outline_mask_ids);
            }

            if let Some(text) = label {
                self.0.ui_labels.push(UiLabel {
                    text,
                    color,
                    target: UiLabelTarget::Position3D(
                        ent_context.world_from_entity.transform_point3(center),
                    ),
                    labeled_instance: instance_hash,
                });
            }
        }

        self.0
            .add_bounding_box(ent_path.hash(), bounding_box, ent_context.world_from_entity);
    }
}

// ---

struct Boxes3DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub half_sizes: &'a [HalfSizes3D],
    pub centers: Option<&'a [Option<Position3D>]>,
    pub rotations: Option<&'a [Option<Rotation3D>]>,
    pub colors: Option<&'a [Option<Color>]>,
    pub radii: Option<&'a [Option<Radius>]>,
    pub labels: Option<&'a [Option<Text>]>,
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
}

impl IdentifiedViewSystem for Boxes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes3D".into()
    }
}

impl VisualizerSystem for Boxes3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Boxes3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let num_boxes = super::entity_iterator::count_instances_in_archetype_views::<
            Boxes3DVisualizer,
            Boxes3D,
            9,
        >(ctx, query) as u32;

        if num_boxes == 0 {
            return Ok(Vec::new());
        }

        // Each box consists of 12 independent lines.
        let mut line_builder =
            LineBatchesBuilder::new(ctx.render_ctx, num_boxes * 12, num_boxes * 12 * 2)
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype_pov1_comp7::<
            Boxes3DVisualizer,
            Boxes3D,
            HalfSizes3D,
            Position3D,
            Rotation3D,
            Color,
            Radius,
            Text,
            re_types::components::KeypointId,
            re_types::components::ClassId,
            _,
        >(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |_ctx,
             ent_path,
             _ent_props,
             ent_context,
             (_time, _row_id),
             instance_keys,
             half_sizes,
             centers,
             rotations,
             colors,
             radii,
             labels,
             keypoint_ids,
             class_ids| {
                let data = Boxes3DComponentData {
                    instance_keys,
                    half_sizes,
                    centers,
                    rotations,
                    colors,
                    radii,
                    labels,
                    keypoint_ids,
                    class_ids,
                };
                self.process_data(&mut line_builder, query, &data, ent_path, ent_context);
                Ok(Vec::new())
            },
        )?;

        Ok(vec![(line_builder.into_draw_data(ctx.render_ctx)?.into())])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
