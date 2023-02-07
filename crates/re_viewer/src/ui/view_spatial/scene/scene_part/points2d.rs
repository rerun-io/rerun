use glam::Mat4;

use re_data_store::{EntityPath, EntityProperties};
use re_log_types::{
    component_types::{ClassId, ColorRGBA, InstanceKey, KeypointId, Label, Point2D, Radius},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::{OptionalSpaceViewEntityHighlight, SpaceViewHighlights, TransformCache, ViewerContext},
    ui::{
        scene::SceneQuery,
        view_spatial::{scene::Keypoints, Label2D, Label2DTarget, SceneSpatial},
        DefaultColor,
    },
};

use super::{instance_path_hash_for_picking, ScenePart};

pub struct Points2DPart;

impl Points2DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &EntityProperties,
        entity_view: &EntityView<Point2D>,
        ent_path: &EntityPath,
        world_from_obj: Mat4,
        entity_highlight: OptionalSpaceViewEntityHighlight<'_>,
    ) -> Result<(), QueryError> {
        scene.num_logged_2d_objects += 1;

        let mut label_batch = Vec::new();
        let max_num_labels = 10;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);

        // If keypoints ids show up we may need to connect them later!
        // We include time in the key, so that the "Visible history" (time range queries) feature works.
        let mut keypoints: Keypoints = Default::default();

        let mut point_batch = scene
            .primitives
            .points
            .batch("2d points")
            .world_from_obj(world_from_obj);

        let visitor = |instance_key: InstanceKey,
                       pos: Point2D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       label: Option<Label>,
                       class_id: Option<ClassId>,
                       keypoint_id: Option<KeypointId>| {
            let instance_hash = instance_path_hash_for_picking(
                ent_path,
                instance_key,
                entity_view,
                props,
                entity_highlight,
            );

            let pos: glam::Vec2 = pos.into();

            let class_description = annotations.class_description(class_id);

            let annotation_info = keypoint_id.map_or_else(
                || class_description.annotation_info(),
                |keypoint_id| {
                    if let Some(class_id) = class_id {
                        keypoints
                            .entry((class_id, 0))
                            .or_insert_with(Default::default)
                            .insert(keypoint_id, pos.extend(0.0));
                    }
                    class_description.annotation_info_with_keypoint(keypoint_id)
                },
            );

            let mut color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
            let mut radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let label = annotation_info.label(label.map(|l| l.0).as_ref());

            SceneSpatial::apply_hover_and_selection_effect(
                &mut radius,
                &mut color,
                entity_highlight.index_highlight(instance_hash.instance_key),
            );

            point_batch
                .add_point_2d(pos)
                .color(color)
                .radius(radius)
                .user_data(instance_hash);

            if let Some(label) = label {
                if label_batch.len() < max_num_labels {
                    label_batch.push(Label2D {
                        text: label,
                        color,
                        target: Label2DTarget::Point(egui::pos2(pos.x, pos.y)),
                        labled_instance: instance_hash,
                    });
                }
            }
        };

        entity_view.visit6(visitor)?;
        drop(point_batch); // Drop batch so we have access to the scene again (batches need to be dropped before starting new ones).

        if label_batch.len() < max_num_labels {
            scene.ui.labels_2d.extend(label_batch.into_iter());
        }

        // Generate keypoint connections if any.
        scene.load_keypoint_connections(ent_path, keypoints, &annotations, props.interactive);

        Ok(())
    }
}

impl ScenePart for Points2DPart {
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
            let entity_highlight = highlights.entity_highlight(ent_path.hash());

            match query_primary_with_history::<Point2D, 7>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    Point2D::name(),
                    InstanceKey::name(),
                    ColorRGBA::name(),
                    Radius::name(),
                    Label::name(),
                    ClassId::name(),
                    KeypointId::name(),
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
