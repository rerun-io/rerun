use glam::Mat4;
use re_data_store::{EntityPath, EntityProperties};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Label, Radius},
    msg_bundle::Component,
    Arrow3D,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{renderer::LineStripFlags, Size};

use crate::{
    misc::{SpaceViewHighlights, TransformCache, ViewerContext},
    ui::{scene::SceneQuery, view_spatial::SceneSpatial, DefaultColor},
};

use super::{instance_path_hash_for_picking, ScenePart};

pub struct Arrows3DPart;

impl Arrows3DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &EntityProperties,
        entity_view: &EntityView<Arrow3D>,
        ent_path: &EntityPath,
        world_from_obj: Mat4,
        highlights: &SpaceViewHighlights,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);

        let entity_highlight = highlights.entity_highlight(ent_path.hash());

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("arrows")
            .world_from_obj(world_from_obj);

        let visitor = |instance_key: InstanceKey,
                       arrow: Arrow3D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       _label: Option<Label>| {
            let instance_hash = instance_path_hash_for_picking(
                ent_path,
                instance_key,
                entity_view,
                props,
                entity_highlight,
            );

            // TODO(andreas): support labels
            // TODO(andreas): support class ids for arrows
            let annotation_info = annotations.class_description(None).annotation_info();
            let mut color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
            //let label = annotation_info.label(label);

            let re_log_types::Arrow3D { origin, vector } = arrow;

            let vector = glam::Vec3::from(vector);
            let origin = glam::Vec3::from(origin);

            let mut radius = radius.map_or(Size::AUTO, |r| Size(r.0));
            let tip_length = LineStripFlags::get_triangle_cap_tip_length(radius.0);
            let vector_len = vector.length();
            let end = origin + vector * ((vector_len - tip_length) / vector_len);

            SceneSpatial::apply_hover_and_selection_effect(
                &mut radius,
                &mut color,
                entity_highlight.index_highlight(instance_hash.instance_key),
            );

            line_batch
                .add_segment(origin, end)
                .radius(radius)
                .color(color)
                .flags(re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE)
                .user_data(instance_hash);
        };

        entity_view.visit4(visitor)?;

        Ok(())
    }
}

impl ScenePart for Arrows3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_scope!("Points2DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };

            match query_primary_with_history::<Arrow3D, 5>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    Arrow3D::name(),
                    InstanceKey::name(),
                    ColorRGBA::name(),
                    Radius::name(),
                    Label::name(),
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
                        highlights,
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
