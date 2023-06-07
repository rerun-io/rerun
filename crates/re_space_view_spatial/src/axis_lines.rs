use egui::Color32;
use re_data_store::EntityPath;
use re_log_types::InstanceKey;
use re_renderer::LineStripSeriesBuilder;

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

pub fn add_axis_lines(
    line_builder: &mut LineStripSeriesBuilder,
    transform: macaw::IsoTransform,
    ent_path: Option<&EntityPath>,
    axis_length: f32,
) {
    use re_renderer::renderer::LineStripFlags;

    // TODO(andreas): It would be nice if could display the semantics (left/right/up) as a tooltip on hover.
    let line_radius = re_renderer::Size::new_scene(axis_length * 0.05);
    let origin = transform.translation();

    let mut line_batch =
        line_builder
            .batch("origin axis")
            .picking_object_id(re_renderer::PickingLayerObjectId(
                ent_path.map_or(0, |p| p.hash64()),
            ));
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
