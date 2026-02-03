use re_sdk_types::image::ImageKind;
use re_sdk_types::{archetypes, blueprint, components};
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem as _, QueryContext, ViewClass as _, ViewStateExt as _,
};

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
        archetypes::TransformAxes3D::descriptor_axis_length().component,
        |ctx| {
            let query_result = ctx.viewer_ctx().lookup_query_result(ctx.view_ctx.view_id);

            // If there is a camera in the scene and it has a pinhole, use the image plane distance to determine the axis length.
            if let Some(length) = query_result
                .tree
                .lookup_result_by_path(ctx.target_entity_path.hash())
                .cloned()
                .and_then(|data_result| {
                    // TODO(andreas): What if there's several camera visualizers?
                    if let Some(camera_visualizer_instruction) = data_result
                        .visualizer_instructions
                        .iter()
                        .find(|instruction| {
                            instruction.visualizer_type
                                == visualizers::CamerasVisualizer::identifier()
                        })
                    {
                        let results = data_result
                            .latest_at_with_blueprint_resolved_data::<archetypes::Pinhole>(
                                ctx.view_ctx,
                                ctx.query,
                                Some(camera_visualizer_instruction),
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

    // Show frame
    system_registry.register_fallback_provider(
        archetypes::TransformAxes3D::descriptor_show_frame().component,
        |_ctx| {
            // We don't show the label with the frame id by default.
            components::ShowLabels(false.into())
        },
    );

    system_registry.register_fallback_provider(
        blueprint::archetypes::SpatialInformation::descriptor_target_frame().component,
        |ctx| {
            // 1. Check if the space root has a defined coordinate frame.
            // 2. Check if all coordinate frames logged on entities included in the filter share the same
            //    root frame, if so use that frame.
            // 3. Use the implicit frame for the space root.

            // Here be dragons: DO NOT use `ctx.query` directly, since we're providing the fallback for a component which only lives in
            // view properties, therefore the `QueryContext` is actually only querying the blueprint directly.
            // However, we're now interested in something that lives on the store but may have an _override_ on the blueprint.
            let query = ctx.view_ctx.current_query();

            let query_result = ctx.view_ctx.query_result;
            let space_origin = ctx.view_ctx.space_origin;
            let is_3d_view =
                ctx.view_ctx.view_class_identifier == crate::SpatialView3D::identifier();

            if let Some(data_result) = query_result.tree.lookup_result_by_path(space_origin.hash())
            {
                let results = data_result
                    .latest_at_with_blueprint_resolved_data::<archetypes::CoordinateFrame>(
                        ctx.view_ctx,
                        &query,
                        None,
                    );

                if let Some(frame_id) = results.get_mono::<components::TransformFrameId>(
                    archetypes::CoordinateFrame::descriptor_frame().component,
                ) {
                    return frame_id;
                }
            }

            'scope: {
                let caches = ctx.store_ctx().caches;
                let (frame_id_registry, transform_forest) =
                    caches.entry(|c: &mut re_viewer_context::TransformDatabaseStoreCache| {
                        (c.frame_id_registry(ctx.recording()), c.transform_forest())
                    });

                let Some(transform_forest) = transform_forest else {
                    break 'scope;
                };

                let mut found_root = None;
                let mut multiple_roots = false;

                query_result.tree.visit(&mut |node| {
                    if multiple_roots {
                        return false;
                    }
                    if node.data_result.tree_prefix_only {
                        return true;
                    }

                    let Some(root_from_frame) = node
                        .data_result
                        .latest_at_with_blueprint_resolved_data_for_component(
                            ctx.view_ctx,
                            &query,
                            archetypes::CoordinateFrame::descriptor_frame().component,
                            None,
                        )
                        .get_mono::<components::TransformFrameId>(
                            archetypes::CoordinateFrame::descriptor_frame().component,
                        )
                        .and_then(|frame| {
                            transform_forest
                                .root_from_frame(re_tf::TransformFrameIdHash::new(&frame))
                        })
                    else {
                        return true;
                    };

                    // If we're in a 3D view, resolve all camera roots to the 3D root they're embedded in.
                    let root_frame_id = if is_3d_view
                        && let Some(pinhole_tree_info) =
                            transform_forest.pinhole_tree_root_info(root_from_frame.root)
                    {
                        pinhole_tree_info.parent_tree_root
                    } else {
                        root_from_frame.root
                    };

                    if let Some(root) = found_root
                        && root != root_frame_id
                    {
                        found_root = None;
                        multiple_roots = true;
                    } else {
                        found_root = Some(root_frame_id);
                    }

                    true
                });

                // Pick the first (alphabetical order) non-entity path root if
                // we can find one.
                if let Some(frame) = found_root
                    && let Some(frame) = frame_id_registry.lookup_frame_id(frame)
                {
                    return frame.clone();
                }
            }

            // Fallback to entity path if no explicit CoordinateFrame
            components::TransformFrameId::from_entity_path(space_origin)
        },
    );
}
