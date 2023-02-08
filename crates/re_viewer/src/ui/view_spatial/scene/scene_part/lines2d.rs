use glam::Mat4;

use re_data_store::{EntityPath, EntityProperties};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, LineStrip2D, Radius},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{renderer::LineStripFlags, Size};

use crate::{
    misc::{OptionalSpaceViewEntityHighlight, SpaceViewHighlights, TransformCache, ViewerContext},
    ui::{scene::SceneQuery, view_spatial::SceneSpatial, DefaultColor},
};

use super::{instance_path_hash_for_picking, ScenePart};

pub struct Lines2DPart;

impl Lines2DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &EntityProperties,
        entity_view: &EntityView<LineStrip2D>,
        ent_path: &EntityPath,
        world_from_obj: Mat4,
        entity_highlight: OptionalSpaceViewEntityHighlight<'_>,
    ) -> Result<(), QueryError> {
        scene.num_logged_2d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("lines 2d")
            .world_from_obj(world_from_obj);

        let visitor = |instance_key: InstanceKey,
                       strip: LineStrip2D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>| {
            let instance_hash = instance_path_hash_for_picking(
                ent_path,
                instance_key,
                entity_view,
                props,
                entity_highlight,
            );

            // TODO(andreas): support class ids for lines
            let annotation_info = annotations.class_description(None).annotation_info();
            let mut radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let mut color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            SceneSpatial::apply_hover_and_selection_effect(
                &mut radius,
                &mut color,
                entity_highlight.index_highlight(instance_hash.instance_key),
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
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };
            let entity_highlight = highlights.entity_highlight(ent_path.hash());

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
                        query,
                        &props,
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
