use re_components::{ClassId, ColorRGBA, Component as _, InstanceKey, Label, Radius, Rect2D};
use re_data_store::EntityPath;
use re_query::{EntityView, QueryError};
use re_renderer::Size;
use re_viewer_context::{
    ArchetypeDefinition, DefaultColor, SceneQuery, SpaceViewHighlights, ViewPartSystem,
    ViewerContext,
};

use crate::{
    scene::{
        contexts::{SpatialSceneContext, SpatialSceneEntityContext},
        parts::entity_iterator::process_entity_views,
        UiLabel, UiLabelTarget,
    },
    SpatialSpaceView,
};

use super::{picking_id_from_instance_key, SpatialScenePartData, SpatialSpaceViewState};

#[derive(Default)]
pub struct Boxes2DPart(SpatialScenePartData);

impl Boxes2DPart {
    fn process_entity_view(
        &mut self,
        _query: &SceneQuery<'_>,
        ent_view: &EntityView<Rect2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let default_color = DefaultColor::EntityPath(ent_path);

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("2d boxes")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_obj)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        ent_view.visit5(
            |instance_key,
             rect,
             color: Option<ColorRGBA>,
             radius: Option<Radius>,
             label: Option<Label>,
             class_id: Option<ClassId>| {
                let instance_hash =
                    re_data_store::InstancePathHash::instance(ent_path, instance_key);

                let annotation_info = ent_context
                    .annotations
                    .class_description(class_id)
                    .annotation_info();
                let color =
                    annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
                let radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
                let label = annotation_info.label(label.map(|l| l.0).as_ref());

                self.0.extend_bounding_box(
                    macaw::BoundingBox {
                        min: glam::Vec2::from(rect.top_left_corner()).extend(0.0),
                        max: (glam::Vec2::from(rect.top_left_corner())
                            + glam::vec2(rect.width(), rect.height()))
                        .extend(0.0),
                    },
                    ent_context.world_from_obj,
                );

                let rectangle = line_batch
                    .add_rectangle_outline_2d(
                        rect.top_left_corner().into(),
                        glam::vec2(rect.width(), 0.0),
                        glam::vec2(0.0, rect.height()),
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

                if let Some(label) = label {
                    self.0.ui_labels.push(UiLabel {
                        text: label,
                        color,
                        target: UiLabelTarget::Rect(egui::Rect::from_min_size(
                            rect.top_left_corner().into(),
                            egui::vec2(rect.width(), rect.height()),
                        )),
                        labeled_instance: instance_hash,
                    });
                }
            },
        )
    }
}

impl ViewPartSystem<SpatialSpaceView> for Boxes2DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![
            Rect2D::name(),
            InstanceKey::name(),
            ColorRGBA::name(),
            Radius::name(),
            Label::name(),
            ClassId::name(),
        ]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &SpatialSpaceViewState,
        scene_context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("Boxes2DPart");

        process_entity_views::<Rect2D, 6, _>(
            ctx,
            query,
            scene_context,
            highlights,
            scene_context.depth_offsets.points,
            self.archetype(),
            |_ctx, ent_path, entity_view, ent_context| {
                self.process_entity_view(query, &entity_view, ent_path, ent_context)
            },
        );

        Vec::new() // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&SpatialScenePartData> {
        Some(&self.0)
    }
}
