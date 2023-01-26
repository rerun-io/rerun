use glam::Mat4;

use re_data_store::{InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Instance, LineStrip3D, Radius},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::{OptionalSpaceViewObjectHighlight, SpaceViewHighlights, ViewerContext},
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::SceneSpatial,
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Lines3DPart;

impl Lines3DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &ObjectProps,
        entity_view: &EntityView<LineStrip3D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
        object_highlight: OptionalSpaceViewObjectHighlight<'_>,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::ObjPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("lines 3d")
            .world_from_obj(world_from_obj);

        let visitor = |instance: Instance,
                       strip: LineStrip3D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>| {
            let instance_hash = {
                if props.interactive {
                    InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                } else {
                    InstanceIdHash::NONE
                }
            };

            let mut radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));

            // TODO(andreas): support class ids for lines
            let annotation_info = annotations.class_description(None).annotation_info();
            let mut color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            SceneSpatial::apply_hover_and_selection_effect(
                &mut radius,
                &mut color,
                object_highlight.index_highlight(instance_hash.instance_index_hash),
            );

            line_batch
                .add_strip(strip.0.into_iter().map(|v| v.into()))
                .radius(radius)
                .color(color)
                .user_data(instance_hash);
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
    ) {
        crate::profile_scope!("Lines3DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };
            let object_highlight = highlights.object_highlight(ent_path.hash());

            match query_primary_with_history::<LineStrip3D, 4>(
                &ctx.log_db.obj_db.arrow_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    LineStrip3D::name(),
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
