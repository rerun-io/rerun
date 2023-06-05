use egui::Color32;
use re_data_store::EntityPath;
use re_log_types::InstanceKey;
use re_renderer::LineStripSeriesBuilder;

use super::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES;

/// Primitives sent off to `re_renderer`.
/// (Some meta information still relevant to ui setup as well)
///
/// TODO(andreas): Right now we're using `re_renderer` data structures for reading (bounding box).
///                 In the future, this will be more limited as we're going to gpu staging data as soon as possible
///                 which is very slow to read. See [#594](https://github.com/rerun-io/rerun/pull/594)
pub struct SceneSpatialPrimitives {
    /// Estimated bounding box of all data in scene coordinates. Accumulated.
    pub(super) bounding_box: macaw::BoundingBox,

    pub num_primitives: usize,

    pub line_strips: LineStripSeriesBuilder,

    pub any_outlines: bool,
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

impl SceneSpatialPrimitives {
    pub fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        Self {
            bounding_box: macaw::BoundingBox::nothing(),
            num_primitives: 0,
            line_strips: LineStripSeriesBuilder::new(re_ctx)
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES),
            any_outlines: false,
        }
    }

    /// bounding box covering the rendered scene
    pub fn bounding_box(&self) -> macaw::BoundingBox {
        self.bounding_box
    }

    /// Number of primitives. Rather arbitrary what counts as a primitive so use this only for heuristic purposes!
    pub fn num_primitives(&self) -> usize {
        let Self {
            bounding_box: _,
            num_primitives,
            line_strips,
            any_outlines: _,
        } = &self;

        line_strips.vertices.len() + num_primitives
    }

    pub fn recalculate_bounding_box(&mut self) {
        re_tracing::profile_function!();

        let Self {
            bounding_box,
            num_primitives: _,
            line_strips,
            any_outlines: _,
        } = self;

        // TODO:
        // for image in images {
        //     let rect = &image.textured_rect;
        //     bounding_box.extend(rect.top_left_corner_position);
        //     bounding_box.extend(rect.top_left_corner_position + rect.extent_u);
        //     bounding_box.extend(rect.top_left_corner_position + rect.extent_v);
        //     bounding_box.extend(rect.top_left_corner_position + rect.extent_v + rect.extent_u);
        // }

        // We don't need a very accurate bounding box, so in order to save some time,
        // we calculate a per batch bounding box for lines and points.
        for (batch, vertex_iter) in line_strips.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            *bounding_box = bounding_box.union(batch_bb.transform_affine3(&batch.world_from_obj));
        }
    }

    pub fn add_axis_lines(
        &mut self,
        transform: macaw::IsoTransform,
        ent_path: Option<&EntityPath>,
        axis_length: f32,
    ) {
        use re_renderer::renderer::LineStripFlags;

        // TODO(andreas): It would be nice if could display the semantics (left/right/up) as a tooltip on hover.
        let line_radius = re_renderer::Size::new_scene(axis_length * 0.05);
        let origin = transform.translation();

        let mut line_batch = self.line_strips.batch("origin axis").picking_object_id(
            re_renderer::PickingLayerObjectId(ent_path.map_or(0, |p| p.hash64())),
        );
        let picking_instance_id = re_renderer::PickingLayerInstanceId(InstanceKey::SPLAT.0);

        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::X) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_X)
            .flags(
                LineStripFlags::FLAG_COLOR_GRADIENT
                    | LineStripFlags::FLAG_CAP_END_TRIANGLE
                    | LineStripFlags::FLAG_CAP_START_ROUND,
            )
            .picking_instance_id(picking_instance_id);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Y) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Y)
            .flags(
                LineStripFlags::FLAG_COLOR_GRADIENT
                    | LineStripFlags::FLAG_CAP_END_TRIANGLE
                    | LineStripFlags::FLAG_CAP_START_ROUND,
            )
            .picking_instance_id(picking_instance_id);
        line_batch
            .add_segment(
                origin,
                origin + transform.transform_vector3(glam::Vec3::Z) * axis_length,
            )
            .radius(line_radius)
            .color(AXIS_COLOR_Z)
            .flags(
                LineStripFlags::FLAG_COLOR_GRADIENT
                    | LineStripFlags::FLAG_CAP_END_TRIANGLE
                    | LineStripFlags::FLAG_CAP_START_ROUND,
            )
            .picking_instance_id(picking_instance_id);
    }
}
