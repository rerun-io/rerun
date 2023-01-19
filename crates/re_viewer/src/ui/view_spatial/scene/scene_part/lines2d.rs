use glam::Mat4;

use re_arrow_store::LatestAtQuery;
use re_data_store::{InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Instance, LineStrip2D, Radius},
    msg_bundle::Component,
};
use re_query::{query_entity_with_primary, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{apply_hover_effect, paint_properties},
            SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Lines2DPart;

impl Lines2DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        _query: &SceneQuery<'_>,
        props: &ObjectProps,
        entity_view: &EntityView<LineStrip2D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
    ) -> Result<(), QueryError> {
        scene.num_logged_2d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::ObjPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("lines 2d")
            .world_from_obj(world_from_obj);

        let highlighted_paths = ctx.hovered().check_obj_path(ent_path.hash());

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

            let stroke_width = radius.map(|r| r.0 * 2.0);

            // TODO(andreas): support class ids for lines
            let annotation_info = annotations.class_description(None).annotation_info();
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            let mut paint_props = paint_properties(color, stroke_width.as_ref());

            if highlighted_paths.contains_index(instance_hash.instance_index_hash) {
                apply_hover_effect(&mut paint_props);
            }

            line_batch
                .add_strip_2d(strip.0.into_iter().map(|v| v.into()))
                .color(paint_props.fg_stroke.color)
                .radius(Size::new_points(paint_props.fg_stroke.width * 0.5))
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
    ) {
        crate::profile_scope!("Lines2DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<LineStrip2D>(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                &[ColorRGBA::name(), Radius::name()],
            )
            .and_then(|entity_view| {
                Self::process_entity_view(
                    scene,
                    ctx,
                    query,
                    &props,
                    &entity_view,
                    ent_path,
                    world_from_obj,
                )
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
