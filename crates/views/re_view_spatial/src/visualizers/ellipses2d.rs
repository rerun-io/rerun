use re_log_types::Instance;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId};
use re_sdk_types::archetypes::Ellipses2D;
use re_sdk_types::components::{Color, HalfSize2D, Position2D, Radius, ShowLabels};
use re_sdk_types::{Archetype as _, ArrowString};
use re_view::{clamped_or, process_color_slice};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewStateExt as _, ViewSystemExecutionError, VisualizerExecutionOutput,
    VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
};

use super::utilities::{LabeledBatch, process_labels};
use super::{SpatialViewVisualizerData, process_radius_slice};
use crate::SpatialViewState;
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::ui::ScreenSpace2D;
use crate::visualizers::UiLabelTarget;

// ---

/// Minimum number of segments used to tessellate each ellipse outline.
///
/// The polyline closes, so we emit `segments + 1` vertices per ellipse.
const MIN_ELLIPSE_SEGMENTS: usize = 32;

/// Reached at a radius of about 3300 pixels.
/// Beyond that the outline error grows, to one pixel at a radius of 13 000 pixels,
/// in exchange for bounding the cost of many large ellipses crossing the view.
const MAX_ELLIPSE_SEGMENTS: usize = 256;

/// Largest distance between outline and true ellipse.
/// A quarter pixel is invisible when anti-aliased.
const MAX_OUTLINE_ERROR_IN_PIXELS: f32 = 0.25;

/// Fewest segments, as a multiple of 4, that keep an outline with largest radius
/// `radius_in_pixels` within [`MAX_OUTLINE_ERROR_IN_PIXELS`].
fn ellipse_segment_count(radius_in_pixels: f32) -> usize {
    if radius_in_pixels.is_nan() || radius_in_pixels <= 0.0 {
        return MIN_ELLIPSE_SEGMENTS;
    }

    // Chord error is `r * (1 - cos(PI / n)) = 2 * r * sin²(PI / 2n)`.
    // `asin` stays precise for large radii, unlike `acos` near 1.
    let half_angle = 2.0
        * (MAX_OUTLINE_ERROR_IN_PIXELS / (2.0 * radius_in_pixels))
            .sqrt()
            .min(1.0)
            .asin();
    let segments = (std::f32::consts::PI / half_angle).ceil();
    (segments as usize)
        .clamp(MIN_ELLIPSE_SEGMENTS, MAX_ELLIPSE_SEGMENTS)
        .next_multiple_of(4)
}

/// Like [`ellipse_segment_count`],
/// but ellipses outside last frame's view keep [`MIN_ELLIPSE_SEGMENTS`].
fn ellipse_segment_count_in_view(
    screen_space: Option<&ScreenSpace2D>,
    world_from_obj: &glam::Affine3A,
    center: glam::Vec2,
    half_size: glam::Vec2,
) -> usize {
    let Some(ScreenSpace2D {
        pixels_per_scene_unit,
        visible_scene_rect,
    }) = screen_space
    else {
        return MIN_ELLIPSE_SEGMENTS;
    };

    let padded_half_size = half_size.abs() * BOUNDING_BOX_PADDING_FACTOR;
    let bbox_in_scene = egui::Rect::from_points(
        &[
            center - padded_half_size,
            center + glam::vec2(padded_half_size.x, -padded_half_size.y),
            center + padded_half_size,
            center + glam::vec2(-padded_half_size.x, padded_half_size.y),
        ]
        .map(|corner| {
            let corner = world_from_obj.transform_point3(corner.extend(0.0));
            egui::pos2(corner.x, corner.y)
        }),
    );
    if !visible_scene_rect.intersects(bbox_in_scene) {
        return MIN_ELLIPSE_SEGMENTS;
    }

    // The ellipse is convex, so if all view corners are inside it, its outline is off screen.
    // Shrink it by the padding so thick outlines near a corner still count as visible.
    let shrunk_half_size = half_size.abs() / BOUNDING_BOX_PADDING_FACTOR;
    let obj_from_world = world_from_obj.inverse();
    let view_is_inside_ellipse = [
        visible_scene_rect.left_top(),
        visible_scene_rect.right_top(),
        visible_scene_rect.right_bottom(),
        visible_scene_rect.left_bottom(),
    ]
    .iter()
    .all(|corner| {
        let corner = obj_from_world.transform_point3(glam::vec3(corner.x, corner.y, 0.0));
        let corner_in_unit_circle = (corner.truncate() - center) / shrunk_half_size;
        corner_in_unit_circle.length_squared() < 1.0
    });
    if view_is_inside_ellipse {
        return MIN_ELLIPSE_SEGMENTS;
    }

    let obj_scale = world_from_obj
        .matrix3
        .x_axis
        .length()
        .max(world_from_obj.matrix3.y_axis.length());
    ellipse_segment_count(half_size.abs().max_element() * obj_scale * pixels_per_scene_unit)
}

/// Extra padding added around the tight ellipse extents when computing the scene bounding box,
/// so auto-fit views don't clip the outline at the edges.
const BOUNDING_BOX_PADDING_FACTOR: f32 = 1.1;

#[derive(Default)]
pub struct Ellipses2DVisualizer;

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Ellipses2DVisualizer {
    fn process_data<'a>(
        view_data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        screen_space: Option<&ScreenSpace2D>,
        data: impl Iterator<Item = Ellipses2DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.half_sizes.len();
            if num_instances == 0 {
                continue;
            }

            let radii = process_radius_slice(
                ctx,
                entity_path,
                num_instances,
                data.line_radii,
                Ellipses2D::descriptor_line_radii().component,
            );
            let colors = process_color_slice(
                ctx,
                Ellipses2D::descriptor_colors().component,
                num_instances,
                data.colors,
            );

            let world_from_obj = ent_context
                .transform_info
                .single_transform_required_for_entity(entity_path, Ellipses2D::name())
                .as_affine3a();

            let mut line_batch = line_builder
                .batch("ellipses2d")
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = re_math::BoundingBox::nothing();

            let centers = clamped_or(data.centers, &Position2D::ZERO);

            for (i, (half_size, center, radius, &color)) in
                itertools::izip!(data.half_sizes, centers, radii, &colors).enumerate()
            {
                let cx = center.x();
                let cy = center.y();
                let rx = half_size.x();
                let ry = half_size.y();

                let padded_half = glam::vec2(rx, ry) * BOUNDING_BOX_PADDING_FACTOR;
                let bbox_center = glam::vec2(cx, cy);
                obj_space_bounding_box.extend((bbox_center - padded_half).extend(0.0));
                obj_space_bounding_box.extend((bbox_center + padded_half).extend(0.0));

                let segments = ellipse_segment_count_in_view(
                    screen_space,
                    &world_from_obj,
                    bbox_center,
                    glam::vec2(rx, ry),
                );
                let delta = std::f32::consts::TAU / segments as f32;
                #[expect(clippy::range_plus_one)] // `RangeInclusive` is not an `ExactSizeIterator`
                let points = (0..segments + 1).map(|n| {
                    let theta = n as f32 * delta;
                    glam::vec2(cx + rx * theta.cos(), cy + ry * theta.sin())
                });

                let ellipse = line_batch
                    .add_strip_2d(points)
                    .flags(LineDrawableBuilder::default_shape_flags())
                    .color(color)
                    .radius(radius)
                    .picking_instance_id(PickingLayerInstanceId(i as _));
                if let Some(outline_mask_ids) = ent_context
                    .highlight
                    .instances
                    .get(&Instance::from(i as u64))
                {
                    ellipse.outline_mask_ids(*outline_mask_ids);
                }
            }

            view_data.add_bounding_box_2d(
                entity_path.hash(),
                obj_space_bounding_box,
                world_from_obj,
            );

            view_data.ui_labels.extend(process_labels(
                LabeledBatch {
                    entity_path,
                    visualizer_instruction: ent_context.visualizer_instruction,
                    num_instances,
                    overall_position: UiLabelTarget::Point2D(
                        <[f32; 2]>::from(obj_space_bounding_box.center().truncate()).into(),
                    ),
                    instance_positions: clamped_or(data.centers, &Position2D::ZERO)
                        .map(|center| UiLabelTarget::Point2D(egui::pos2(center.x(), center.y()))),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| {
                        typed_fallback_for(ctx, Ellipses2D::descriptor_show_labels().component)
                    }),
                },
                std::convert::identity,
            ));
        }
    }
}

// ---

struct Ellipses2DComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSize2D],

    // Clamped to edge
    centers: &'a [Position2D],
    colors: &'a [Color],
    line_radii: &'a [Radius],
    labels: Vec<ArrowString>,

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Ellipses2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Ellipses2D".into()
    }
}

impl VisualizerSystem for Ellipses2DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<HalfSize2D>(
            &Ellipses2D::descriptor_half_sizes(),
            &Ellipses2D::all_components(),
        )
        .with_annotation_context(re_viewer_context::AnnotationContextQuery::new(
            Ellipses2D::descriptor_class_ids().component,
            [
                re_viewer_context::AnnotationContextTarget::color(Ellipses2D::descriptor_colors()),
                re_viewer_context::AnnotationContextTarget::label(Ellipses2D::descriptor_labels()),
            ],
        ))
    }

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        Some(crate::SpatialView2D::identifier())
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let screen_space = ctx
            .view_state
            .downcast_ref::<SpatialViewState>()
            .ok()
            .and_then(|state| state.screen_space_2d);

        let mut view_data = SpatialViewVisualizerData::default();
        let output = VisualizerExecutionOutput::default();
        let mut line_builder = LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Ellipses2D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_half_sizes =
                    results.iter_required(Ellipses2D::descriptor_half_sizes().component);
                if all_half_sizes.is_empty() {
                    return Ok(());
                }

                let num_ellipses: usize = all_half_sizes
                    .chunks()
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 2]>())
                    .map(|vectors| vectors.len())
                    .sum();
                if num_ellipses == 0 {
                    return Ok(());
                }

                // Each ellipse is one closed strip with at least `MIN_ELLIPSE_SEGMENTS + 1` vertices.
                line_builder.reserve_strips(num_ellipses)?;
                line_builder.reserve_vertices(num_ellipses * (MIN_ELLIPSE_SEGMENTS + 1))?;

                let all_centers = results.iter_optional(Ellipses2D::descriptor_centers().component);
                let all_colors = results.iter_optional(Ellipses2D::descriptor_colors().component);
                let all_line_radii =
                    results.iter_optional(Ellipses2D::descriptor_line_radii().component);
                let all_labels = results.iter_optional(Ellipses2D::descriptor_labels().component);
                let all_show_labels =
                    results.iter_optional(Ellipses2D::descriptor_show_labels().component);

                let results_iter = re_query::range_zip_1x5(
                    all_half_sizes.slice::<[f32; 2]>(),
                    all_centers.slice::<[f32; 2]>(),
                    all_colors.slice::<u32>(),
                    all_line_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(_index, half_sizes, centers, colors, line_radii, labels, show_labels)| {
                        Ellipses2DComponentData {
                            half_sizes: bytemuck::cast_slice(half_sizes),
                            centers: centers.map_or(&[], |centers| bytemuck::cast_slice(centers)),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            line_radii: line_radii
                                .map_or(&[], |line_radii| bytemuck::cast_slice(line_radii)),
                            labels: labels.unwrap_or_default(),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
                        }
                    },
                );

                Self::process_data(
                    &mut view_data,
                    ctx,
                    &mut line_builder,
                    spatial_ctx,
                    screen_space.as_ref(),
                    results_iter,
                );

                Ok(())
            },
        )?;

        Ok(output
            .with_draw_data([(line_builder.into_draw_data()?.into())])
            .with_visualizer_data(view_data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Largest distance between the tessellated outline and the true circle.
    fn max_error_in_pixels(radius_in_pixels: f32, segments: usize) -> f64 {
        f64::from(radius_in_pixels) * (1.0 - (std::f64::consts::PI / segments as f64).cos())
    }

    /// A 2D view showing scene coordinates -50..50 on both axes, at 10 pixels per unit.
    fn screen_space() -> ScreenSpace2D {
        ScreenSpace2D {
            pixels_per_scene_unit: 10.0,
            visible_scene_rect: egui::Rect::from_min_max(
                egui::pos2(-50.0, -50.0),
                egui::pos2(50.0, 50.0),
            ),
        }
    }

    #[test]
    fn small_ellipses_use_minimum_segment_count() {
        assert_eq!(ellipse_segment_count(0.0), MIN_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(0.5), MIN_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(10.0), MIN_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(-3.0), MIN_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(f32::MIN), MIN_ELLIPSE_SEGMENTS);
        assert_eq!(
            ellipse_segment_count(f32::NEG_INFINITY),
            MIN_ELLIPSE_SEGMENTS
        );
        assert_eq!(
            ellipse_segment_count(f32::MIN_POSITIVE),
            MIN_ELLIPSE_SEGMENTS
        );
        assert_eq!(ellipse_segment_count(1e-30), MIN_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(f32::NAN), MIN_ELLIPSE_SEGMENTS);
    }

    #[test]
    fn without_a_2d_view_use_the_default_count() {
        let segments = ellipse_segment_count_in_view(
            None,
            &glam::Affine3A::IDENTITY,
            glam::Vec2::ZERO,
            glam::vec2(100.0, 100.0),
        );
        assert_eq!(segments, MIN_ELLIPSE_SEGMENTS);
    }

    #[test]
    fn visible_ellipses_follow_screen_size() {
        let screen_space = screen_space();
        let segments = |world_from_obj: glam::Affine3A, center: glam::Vec2| {
            ellipse_segment_count_in_view(
                Some(&screen_space),
                &world_from_obj,
                center,
                glam::vec2(100.0, 50.0),
            )
        };

        // Radius 100 at 10 px per unit is 1000 px.
        assert_eq!(segments(glam::Affine3A::IDENTITY, glam::Vec2::ZERO), 144);
        // Straddling the right edge.
        assert_eq!(
            segments(glam::Affine3A::IDENTITY, glam::vec2(140.0, 0.0)),
            144
        );
        // The entity transform scales the radius to 2000 px.
        // Straddling the right edge like above (center 70 * 2 = 140).
        assert_eq!(
            segments(
                glam::Affine3A::from_scale(glam::vec3(2.0, 2.0, 1.0)),
                glam::vec2(70.0, 0.0)
            ),
            200
        );
    }

    #[test]
    fn off_screen_ellipses_use_the_default_count() {
        let screen_space = screen_space();
        let segments = |world_from_obj: glam::Affine3A, center: glam::Vec2| {
            ellipse_segment_count_in_view(
                Some(&screen_space),
                &world_from_obj,
                center,
                glam::vec2(100.0, 50.0),
            )
        };

        assert_eq!(
            segments(glam::Affine3A::IDENTITY, glam::vec2(1000.0, 0.0)),
            MIN_ELLIPSE_SEGMENTS
        );
        assert_eq!(
            segments(glam::Affine3A::IDENTITY, glam::vec2(0.0, -1000.0)),
            MIN_ELLIPSE_SEGMENTS
        );
        // Moved off screen by the entity transform.
        assert_eq!(
            segments(
                glam::Affine3A::from_translation(glam::vec3(1000.0, 0.0, 0.0)),
                glam::Vec2::ZERO
            ),
            MIN_ELLIPSE_SEGMENTS
        );
    }

    #[test]
    fn ellipses_around_the_view_use_the_default_count() {
        let screen_space = screen_space();
        let segments = |world_from_obj: glam::Affine3A, half_size: glam::Vec2| {
            ellipse_segment_count_in_view(
                Some(&screen_space),
                &world_from_obj,
                glam::Vec2::ZERO,
                half_size,
            )
        };

        assert_eq!(
            segments(glam::Affine3A::IDENTITY, glam::vec2(1000.0, 500.0)),
            MIN_ELLIPSE_SEGMENTS
        );
        // Enlarged around the view by the entity transform.
        assert_eq!(
            segments(
                glam::Affine3A::from_scale(glam::vec3(10.0, 10.0, 1.0)),
                glam::vec2(100.0, 50.0)
            ),
            MIN_ELLIPSE_SEGMENTS
        );
        // The outline passes just outside the view's corners, where a thick line still shows.
        assert!(segments(glam::Affine3A::IDENTITY, glam::vec2(72.0, 72.0)) > MIN_ELLIPSE_SEGMENTS);
    }

    #[test]
    fn huge_ellipses_use_maximum_segment_count() {
        assert_eq!(ellipse_segment_count(1e9), MAX_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(1e30), MAX_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(f32::MAX), MAX_ELLIPSE_SEGMENTS);
        assert_eq!(ellipse_segment_count(f32::INFINITY), MAX_ELLIPSE_SEGMENTS);
    }

    #[test]
    fn segment_count_follows_screen_size() {
        // π / acos(1 - 0.25 / 1000) ≈ 140.5, rounded up to a multiple of 4.
        assert_eq!(ellipse_segment_count(1000.0), 144);
    }

    #[test]
    fn outline_stays_within_error_bound() {
        let mut previous = 0;
        for radius in (1..20_000).map(|r| r as f32 * 0.5) {
            let segments = ellipse_segment_count(radius);
            assert_eq!(segments % 4, 0, "radius {radius}: {segments} segments");
            assert!(segments >= previous, "radius {radius}: count went down");
            if segments < MAX_ELLIPSE_SEGMENTS {
                assert!(
                    max_error_in_pixels(radius, segments) <= 0.25,
                    "radius {radius}: {segments} segments is too coarse"
                );
            }
            previous = segments;
        }
    }
}
