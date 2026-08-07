// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A video binary.
///
/// Only MP4 containers are currently supported.
///
/// See <https://rerun.io/docs/reference/video> for codec support and more general information.
///
/// In order to display a video, you also need to log a [archetypes.VideoFrameReference] for each frame.
///
/// \example archetypes/video_auto_frames title="Video with automatically determined frames" image="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/1200w.png"
/// \example archetypes/video_manual_frames title="Demonstrates manual use of video frame references" image="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Video")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer_none)]
pub struct AssetVideo {
    /// The asset's bytes.
    #[rerun(component_no_ui_edit)]
    #[rerun(component_required)]
    pub blob: rerun::components::Blob,

    /// The Media Type of the asset.
    ///
    /// Supported values:
    /// * `video/mp4`
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to render the asset.
    #[rerun(component_recommended)]
    pub media_type: Option<rerun::components::MediaType>,
}
