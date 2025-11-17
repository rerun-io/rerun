use re_types::{archetypes, components, image::ImageKind};
use re_view::DataResultQuery as _;
use re_viewer_context::{IdentifiedViewSystem as _, QueryContext, ViewStateExt as _};

use crate::{SpatialViewState, visualizers};

fn opacity_fallback(image_kind: ImageKind) -> impl Fn(&QueryContext<'_>) -> components::Opacity {
    move |ctx| {
        // Color images should be transparent whenever they're on top of other images,
        // But fully opaque if there are no other images in the scene.
        let Some(view_state) = ctx.view_state().as_any().downcast_ref::<SpatialViewState>() else {
            return 1.0.into();
        };

        // Known cosmetic issues with this approach:
        // * The first frame we have more than one image, the image will be opaque.
        //      It's too complex to do a full view query just for this here.
        //      However, we should be able to analyze the `DataQueryResults` instead to check how many entities are fed to the Image/DepthImage visualizers.
        // * In 3D scenes, images that are on a completely different plane will cause this to become transparent.
        components::Opacity::from(view_state.fallback_opacity_for_image_kind(image_kind))
    }
}

pub fn register_fallbacks(system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>) {
    // Image opacities
    for component in [
        archetypes::Image::descriptor_opacity().component,
        archetypes::EncodedImage::descriptor_opacity().component,
        archetypes::VideoStream::descriptor_opacity().component,
        archetypes::VideoFrameReference::descriptor_opacity().component,
    ] {
        system_registry.register_fallback_provider(component, opacity_fallback(ImageKind::Color));
    }

    system_registry.register_fallback_provider(
        archetypes::SegmentationImage::descriptor_opacity().component,
        opacity_fallback(ImageKind::Segmentation),
    );

    // Pinhole
    system_registry.register_fallback_provider(
        archetypes::Pinhole::descriptor_image_plane_distance().component,
        |ctx| {
            let Ok(state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
                return Default::default();
            };

            let scene_size = state.bounding_boxes.smoothed.size().length();

            let d = if scene_size.is_finite() && scene_size > 0.0 {
                // Works pretty well for `examples/python/open_photogrammetry_format/open_photogrammetry_format.py --no-frames`
                scene_size * 0.02
            } else {
                // This value somewhat arbitrary. In almost all cases where the scene has defined bounds
                // the heuristic will change it or it will be user edited. In the case of non-defined bounds
                // this value works better with the default camera setup.
                0.3
            };

            components::ImagePlaneDistance::from(d)
        },
    );

    // Axis length
    system_registry.register_fallback_provider(
        archetypes::TransformArrows3D::descriptor_axis_length().component,
        |ctx| {
            let query_result = ctx.viewer_ctx().lookup_query_result(ctx.view_ctx.view_id);

            // If there is a camera in the scene and it has a pinhole, use the image plane distance to determine the axis length.
            if let Some(length) = query_result
                .tree
                .lookup_result_by_path(ctx.target_entity_path.hash())
                .cloned()
                .and_then(|data_result| {
                    if data_result
                        .visualizers
                        .contains(&visualizers::CamerasVisualizer::identifier())
                    {
                        let results = data_result
                            .latest_at_with_blueprint_resolved_data::<archetypes::Pinhole>(
                                ctx.view_ctx,
                                ctx.query,
                            );

                        Some(
                            results.get_mono_with_fallback::<components::ImagePlaneDistance>(
                                archetypes::Pinhole::descriptor_image_plane_distance().component,
                            ),
                        )
                    } else {
                        None
                    }
                })
            {
                let length: f32 = length.into();
                return (length * 0.5).into();
            }

            // If there is a finite bounding box, use the scene size to determine the axis length.
            if let Ok(state) = ctx.view_state().downcast_ref::<SpatialViewState>() {
                let scene_size = state.bounding_boxes.smoothed.size().length();

                if scene_size.is_finite() && scene_size > 0.0 {
                    return (scene_size * 0.05).into();
                }
            }

            // Otherwise 0.3 is a reasonable default.

            // This value somewhat arbitrary. In almost all cases where the scene has defined bounds
            // the heuristic will change it or it will be user edited. In the case of non-defined bounds
            // this value works better with the default camera setup.
            components::AxisLength::from(0.3)
        },
    );
}
