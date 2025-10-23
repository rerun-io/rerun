use re_types::{components, image::ImageKind};
use re_viewer_context::{QueryContext, ViewStateExt as _};

use crate::SpatialViewState;

pub fn opacity_fallback(
    image_kind: ImageKind,
) -> impl Fn(&QueryContext<'_>) -> components::Opacity {
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

pub fn image_plane_distance_fallback(ctx: &QueryContext<'_>) -> components::ImagePlaneDistance {
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
}
