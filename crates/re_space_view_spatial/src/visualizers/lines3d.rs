use re_entity_db::{EntityPath, InstancePathHash};
use re_query::{ArchetypeView, QueryError};
use re_types::{
    archetypes::LineStrips3D,
    components::{Color, LineStrip3D, Radius, Text},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    IdentifiedViewSystem, ResolvedAnnotationInfos, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewQuery, ViewerContext, VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{
        entity_iterator::process_archetype_views, process_annotations, process_colors,
        process_radii, UiLabel, UiLabelTarget,
    },
};

use super::{picking_id_from_instance_key, SpatialViewVisualizerData};

pub struct Lines3DVisualizer {
    /// If the number of arrows in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Lines3DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::ThreeD)),
        }
    }
}

impl Lines3DVisualizer {
    fn process_labels<'a>(
        arch_view: &'a ArchetypeView<LineStrips3D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        re_tracing::profile_function!();

        let labels = itertools::izip!(
            annotation_infos.iter(),
            arch_view.iter_required_component::<LineStrip3D>()?,
            arch_view.iter_optional_component::<Text>()?,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, strip, label, color, labeled_instance)| {
                let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));
                match (strip, label) {
                    (strip, Some(label)) => {
                        let midpoint = strip
                            .0
                            .iter()
                            .copied()
                            .map(glam::Vec3::from)
                            .sum::<glam::Vec3>()
                            / (strip.0.len() as f32);
                        Some(UiLabel {
                            text: label,
                            color: *color,
                            target: UiLabelTarget::Position3D(
                                world_from_obj.transform_point3(midpoint),
                            ),
                            labeled_instance: *labeled_instance,
                        })
                    }
                    _ => None,
                }
            },
        );
        Ok(labels)
    }

    fn process_arch_view(
        &mut self,
        query: &ViewQuery<'_>,
        arch_view: &ArchetypeView<LineStrips3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let annotation_infos = process_annotations::<LineStrip3D, LineStrips3D>(
            query,
            arch_view,
            &ent_context.annotations,
        )?;

        let colors = process_colors(
            arch_view.iter_optional_component::<Color>()?,
            ent_path,
            &annotation_infos,
        );
        let radii = process_radii(arch_view.iter_optional_component::<Radius>()?, ent_path);

        if arch_view.num_instances() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors = process_colors(
                arch_view.iter_optional_component::<Color>()?,
                ent_path,
                &annotation_infos,
            )
            .collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                arch_view
                    .iter_instance_keys()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            self.data.ui_labels.extend(Self::process_labels(
                arch_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                ent_context.world_from_entity,
            )?);
        }

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("lines 3d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let instance_keys = arch_view.iter_instance_keys();
        let pick_ids = arch_view
            .iter_instance_keys()
            .map(picking_id_from_instance_key);
        let strips = arch_view.iter_required_component::<LineStrip3D>()?;

        let mut bounding_box = macaw::BoundingBox::nothing();

        for (instance_key, strip, radius, color, pick_id) in
            itertools::izip!(instance_keys, strips, radii, colors, pick_ids)
        {
            let lines = line_batch
                .add_strip(strip.0.iter().copied().map(Into::into))
                .color(color)
                .radius(radius)
                .picking_instance_id(pick_id);

            if let Some(outline_mask_ids) = ent_context.highlight.instances.get(&instance_key) {
                lines.outline_mask_ids(*outline_mask_ids);
            }

            for p in strip.0 {
                bounding_box.extend(p.into());
            }
        }

        self.data
            .extend_bounding_box(bounding_box, ent_context.world_from_entity);

        Ok(())
    }
}

impl IdentifiedViewSystem for Lines3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Lines3D".into()
    }
}

impl VisualizerSystem for Lines3DVisualizer {
    fn required_components(&self) -> ComponentNameSet {
        LineStrips3D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(LineStrips3D::indicator().name()).collect()
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        process_archetype_views::<
            Lines3DVisualizer,
            LineStrips3D,
            { LineStrips3D::NUM_COMPONENTS },
            _,
        >(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |_ctx, ent_path, _ent_props, arch_view, ent_context| {
                self.process_arch_view(query, &arch_view, ent_path, ent_context)
            },
        )?;

        Ok(Vec::new()) // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
