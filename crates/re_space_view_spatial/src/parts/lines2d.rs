use re_data_store::{EntityPath, InstancePathHash};
use re_query::{ArchetypeView, QueryError};
use re_types::{
    archetypes::LineStrips2D,
    components::{Label, LineStrip2D},
    Archetype as _,
};
use re_viewer_context::{
    ArchetypeDefinition, ResolvedAnnotationInfo, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::{
        entity_iterator::process_archetype_views, process_annotations_and_keypoints,
        process_colors, process_radii, UiLabel, UiLabelTarget,
    },
    view_kind::SpatialSpaceViewKind,
};

use super::{picking_id_from_instance_key, SpatialViewPartData};

pub struct Lines2DPart {
    /// If the number of arrows in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewPartData,
}

impl Default for Lines2DPart {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewPartData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl Lines2DPart {
    fn process_labels<'a>(
        arch_view: &'a ArchetypeView<LineStrips2D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a [ResolvedAnnotationInfo],
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        let labels = itertools::izip!(
            annotation_infos.iter(),
            arch_view.iter_required_component::<LineStrip2D>()?,
            arch_view.iter_optional_component::<Label>()?,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, strip, label, color, labeled_instance)| {
                let label = annotation_info.label(label.map(|l| l.0).as_ref());
                match (strip, label) {
                    (strip, Some(label)) => {
                        let midpoint = strip
                            .0
                            .iter()
                            .copied()
                            .map(glam::Vec2::from)
                            .sum::<glam::Vec2>()
                            / (strip.0.len() as f32);
                        Some(UiLabel {
                            text: label,
                            color: *color,
                            target: UiLabelTarget::Point2D(egui::pos2(midpoint.x, midpoint.y)),
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
        arch_view: &ArchetypeView<LineStrips2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let (annotation_infos, _) = process_annotations_and_keypoints::<LineStrip2D, LineStrips2D>(
            query,
            arch_view,
            &ent_context.annotations,
            |strip| {
                let pos = strip.0.get(0).copied().unwrap_or_default();
                glam::Vec3::new(pos.x(), pos.y(), 0.0)
            },
        )?;

        let colors = process_colors(arch_view, ent_path, &annotation_infos)?;
        let radii = process_radii(arch_view, ent_path)?;

        if arch_view.num_instances() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors =
                process_colors(arch_view, ent_path, &annotation_infos)?.collect::<Vec<_>>();

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
            )?);
        }

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("lines 2d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_obj)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let instance_keys = arch_view.iter_instance_keys();
        let pick_ids = arch_view
            .iter_instance_keys()
            .map(picking_id_from_instance_key);
        let strips = arch_view.iter_required_component::<LineStrip2D>()?;

        let mut bounding_box = macaw::BoundingBox::nothing();

        for (instance_key, strip, radius, color, pick_id) in
            itertools::izip!(instance_keys, strips, radii, colors, pick_ids)
        {
            let lines = line_batch
                .add_strip_2d(strip.0.iter().copied().map(Into::into))
                .color(color)
                .radius(radius)
                .picking_instance_id(pick_id);

            if let Some(outline_mask_ids) = ent_context.highlight.instances.get(&instance_key) {
                lines.outline_mask_ids(*outline_mask_ids);
            }

            for p in strip.0 {
                bounding_box.extend(glam::vec3(p.x(), p.y(), 0.0));
            }
        }

        self.data
            .extend_bounding_box(bounding_box, ent_context.world_from_obj);

        Ok(())
    }
}

impl ViewPartSystem for Lines2DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        LineStrips2D::all_components().try_into().unwrap()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_scope!("Lines2DPart");

        process_archetype_views::<LineStrips2D, { LineStrips2D::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |_ctx, ent_path, arch_view, ent_context| {
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
