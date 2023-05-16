use re_data_store::EntityPath;
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, LineStrip2D, Radius},
    Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::Size;
use re_viewer_context::{DefaultColor, SceneQuery, ViewerContext};

use crate::{
    misc::{SpaceViewHighlights, SpaceViewOutlineMasks, TransformCache},
    ui::view_spatial::{scene::EntityDepthOffsets, SceneSpatial},
};

use super::{instance_key_to_picking_id, ScenePart};

pub struct Lines2DPart;

impl Lines2DPart {
    fn process_entity_view(
        scene: &mut SceneSpatial,
        entity_view: &EntityView<LineStrip2D>,
        ent_path: &EntityPath,
        world_from_obj: glam::Affine3A,
        entity_highlight: &SpaceViewOutlineMasks,
        depth_offset: re_renderer::DepthOffset,
    ) -> Result<(), QueryError> {
        scene.num_logged_2d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("lines 2d")
            .depth_offset(depth_offset)
            .world_from_obj(world_from_obj)
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let visitor = |instance_key: InstanceKey,
                       strip: LineStrip2D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>| {
            // TODO(andreas): support class ids for lines
            let annotation_info = annotations.class_description(None).annotation_info();
            let radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            let lines = line_batch
                .add_strip_2d(strip.0.into_iter().map(|v| v.into()))
                .color(color)
                .radius(radius)
                .picking_instance_id(instance_key_to_picking_id(
                    instance_key,
                    entity_view.num_instances(),
                    entity_highlight.any_selection_highlight,
                ));

            if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance_key) {
                lines.outline_mask_ids(*outline_mask_ids);
            }
        };

        entity_view.visit3(visitor)?;

        Ok(())
    }
}

impl ScenePart for Lines2DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
        depth_offsets: &EntityDepthOffsets,
    ) {
        crate::profile_scope!("Lines2DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };
            let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

            match query_primary_with_history::<LineStrip2D, 4>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    LineStrip2D::name(),
                    InstanceKey::name(),
                    ColorRGBA::name(),
                    Radius::name(),
                ],
            )
            .and_then(|entities| {
                for entity in entities {
                    Self::process_entity_view(
                        scene,
                        &entity,
                        ent_path,
                        world_from_obj,
                        entity_highlight,
                        depth_offsets.get(ent_path).unwrap_or(depth_offsets.lines2d),
                    )?;
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                }
            }
        }
    }
}
