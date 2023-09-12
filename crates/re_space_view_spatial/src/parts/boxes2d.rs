use re_data_store::EntityPath;
use re_query::{ArchetypeView, QueryError};
use re_types::{
    archetypes::Boxes2D,
    components::{HalfSizes2D, Origin2D},
    Archetype,
};
use re_viewer_context::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::{UiLabel, UiLabelTarget},
    view_kind::SpatialSpaceViewKind,
};

use super::{
    entity_iterator::process_archetype_views, picking_id_from_instance_key, process_annotations,
    process_colors, process_radii, SpatialViewPartData,
};

pub struct Boxes2DPart(SpatialViewPartData);

impl Default for Boxes2DPart {
    fn default() -> Self {
        Self(SpatialViewPartData::new(Some(SpatialSpaceViewKind::TwoD)))
    }
}

impl Boxes2DPart {
    fn process_arch_view(
        &mut self,
        query: &ViewQuery<'_>,
        arch_view: &ArchetypeView<Boxes2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let annotation_infos = process_annotations::<HalfSizes2D, Boxes2D>(
            query,
            arch_view,
            &ent_context.annotations,
        )?;

        let instance_keys = arch_view.iter_instance_keys();
        let half_sizes = arch_view.iter_required_component::<HalfSizes2D>()?;
        let origins = arch_view
            .iter_optional_component::<Origin2D>()?
            .map(|origin| origin.unwrap_or_default());
        let radii = process_radii(arch_view, ent_path)?;
        let colors = process_colors(arch_view, ent_path, &annotation_infos)?;
        let labels = arch_view.iter_optional_component::<re_types::components::Text>()?;

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("boxes2d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_obj)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        for (instance_key, half_extent, origin, radius, color, label) in
            itertools::izip!(instance_keys, half_sizes, origins, radii, colors, labels)
        {
            let instance_hash = re_data_store::InstancePathHash::instance(ent_path, instance_key);

            let min = half_extent.box_min(origin);
            let max = half_extent.box_max(origin);

            self.0.extend_bounding_box(
                macaw::BoundingBox {
                    min: min.extend(0.),
                    max: max.extend(0.),
                },
                ent_context.world_from_obj,
            );

            let rectangle = line_batch
                .add_rectangle_outline_2d(
                    min,
                    glam::vec2(half_extent.width(), 0.0),
                    glam::vec2(0.0, half_extent.height()),
                )
                .color(color)
                .radius(radius)
                .picking_instance_id(picking_id_from_instance_key(instance_key));
            if let Some(outline_mask_ids) = ent_context
                .highlight
                .instances
                .get(&instance_hash.instance_key)
            {
                rectangle.outline_mask_ids(*outline_mask_ids);
            }

            if let Some(text) = label {
                self.0.ui_labels.push(UiLabel {
                    text: text.to_string(),
                    color,
                    target: UiLabelTarget::Rect(egui::Rect::from_min_max(
                        egui::pos2(min.x, min.y),
                        egui::pos2(max.x, max.y),
                    )),
                    labeled_instance: instance_hash,
                });
            }
        }

        Ok(())
    }
}

impl NamedViewSystem for Boxes2DPart {
    fn name() -> re_viewer_context::ViewSystemName {
        "Boxes2D".into()
    }
}

impl ViewPartSystem for Boxes2DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        Boxes2D::all_components().try_into().unwrap()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        process_archetype_views::<Boxes2DPart, Boxes2D, { Boxes2D::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.box2d,
            |_ctx, ent_path, arch_view, ent_context| {
                self.process_arch_view(query, &arch_view, ent_path, ent_context)
            },
        )?;

        Ok(Vec::new()) // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
