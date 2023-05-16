use re_data_store::EntityPath;
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, LineStrip3D, Radius},
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

pub struct Lines3DPart;

impl Lines3DPart {
    fn process_entity_view(
        scene: &mut SceneSpatial,
        entity_view: &EntityView<LineStrip3D>,
        ent_path: &EntityPath,
        world_from_obj: glam::Affine3A,
        entity_highlight: &SpaceViewOutlineMasks,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);
        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("lines 3d")
            .world_from_obj(world_from_obj)
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let visitor = |instance_key: InstanceKey,
                       strip: LineStrip3D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>| {
            let radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));

            // TODO(andreas): support class ids for lines
            let annotation_info = annotations.class_description(None).annotation_info();
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            let lines = line_batch
                .add_strip(strip.0.into_iter().map(|v| v.into()))
                .radius(radius)
                .color(color)
                .flags(re_renderer::renderer::LineStripFlags::FLAG_COLOR_GRADIENT)
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

impl ScenePart for Lines3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
        _depth_offsets: &EntityDepthOffsets,
    ) {
        crate::profile_scope!("Lines3DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };
            let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

            match query_primary_with_history::<LineStrip3D, 4>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    LineStrip3D::name(),
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
