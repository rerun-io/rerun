use re_data_store::EntityPath;
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Label, Radius},
    Arrow3D, Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{renderer::LineStripFlags, Size};
use re_viewer_context::{DefaultColor, SceneQuery, ViewerContext};

use crate::{
    misc::{SpaceViewHighlights, TransformCache},
    ui::view_spatial::{scene::EntityDepthOffsets, SceneSpatial},
};

use super::{instance_key_to_picking_id, ScenePart};

pub struct Arrows3DPart;

impl Arrows3DPart {
    fn process_entity_view(
        scene: &mut SceneSpatial,
        entity_view: &EntityView<Arrow3D>,
        ent_path: &EntityPath,
        world_from_obj: glam::Affine3A,
        highlights: &SpaceViewHighlights,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);

        let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("arrows")
            .world_from_obj(world_from_obj)
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let visitor = |instance_key: InstanceKey,
                       arrow: Arrow3D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       _label: Option<Label>| {
            // TODO(andreas): support labels
            // TODO(andreas): support class ids for arrows
            let annotation_info = annotations.class_description(None).annotation_info();
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
            //let label = annotation_info.label(label);

            let re_log_types::Arrow3D { origin, vector } = arrow;

            let vector = glam::Vec3::from(vector);
            let origin = glam::Vec3::from(origin);

            let radius = radius.map_or(Size::AUTO, |r| Size(r.0));
            let end = origin + vector;

            let segment = line_batch
                .add_segment(origin, end)
                .radius(radius)
                .color(color)
                .flags(
                    LineStripFlags::FLAG_COLOR_GRADIENT
                        | LineStripFlags::FLAG_CAP_END_TRIANGLE
                        | LineStripFlags::FLAG_CAP_START_ROUND
                        | LineStripFlags::FLAG_CAP_START_EXTEND_OUTWARDS,
                )
                .picking_instance_id(instance_key_to_picking_id(
                    instance_key,
                    entity_view.num_instances(),
                    entity_highlight.any_selection_highlight,
                ));

            if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance_key) {
                segment.outline_mask_ids(*outline_mask_ids);
            }
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
        _depth_offsets: &EntityDepthOffsets,
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
