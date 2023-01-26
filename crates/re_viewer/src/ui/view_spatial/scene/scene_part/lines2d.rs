use glam::Mat4;

use re_data_store::{InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Instance, LineStrip2D, Radius},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{renderer::LineStripFlags, Size};

use crate::{
    misc::{OptionalSpaceViewObjectHighlight, SpaceViewHighlights, ViewerContext},
    ui::{
        scene::SceneQuery, transform_cache::TransformCache, view_spatial::SceneSpatial,
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Lines2DPart;

impl Lines2DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &ObjectProps,
        entity_view: &EntityView<LineStrip2D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
        object_highlight: OptionalSpaceViewObjectHighlight<'_>,
    ) -> Result<(), QueryError> {
        scene.num_logged_2d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::ObjPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("lines 2d")
            .world_from_obj(world_from_obj);

        let visitor = |instance: Instance,
                       strip: LineStrip2D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>| {
            let instance_hash = {
                if props.interactive {
                    InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                } else {
                    InstanceIdHash::NONE
                }
            };

            // TODO(andreas): support class ids for lines
            let annotation_info = annotations.class_description(None).annotation_info();
            let mut radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let mut color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            SceneSpatial::apply_hover_and_selection_effect(
                &mut radius,
                &mut color,
                object_highlight.index_highlight(instance_hash.instance_index_hash),
            );

            line_batch
                .add_strip_2d(strip.0.into_iter().map(|v| v.into()))
                .color(color)
                .radius(radius)
                .flags(LineStripFlags::NO_COLOR_GRADIENT)
                .user_data(instance_hash);
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
    ) {
        crate::profile_scope!("Lines2DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };
            let object_highlight = highlights.object_highlight(ent_path.hash());

            match query_primary_with_history::<LineStrip2D, 4>(
                &ctx.log_db.obj_db.arrow_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    LineStrip2D::name(),
                    Instance::name(),
                    ColorRGBA::name(),
                    Radius::name(),
                ],
            )
            .and_then(|entities| {
                for entity in entities {
                    Self::process_entity_view(
                        scene,
                        query,
                        &props,
                        &entity,
                        ent_path,
                        world_from_obj,
                        object_highlight,
                    )?;
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
