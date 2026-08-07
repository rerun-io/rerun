// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Video stream consisting of raw video chunks.
///
/// For logging video containers like mp4, refer to [archetypes.AssetVideo] and [archetypes.VideoFrameReference].
/// To learn more about video support in Rerun, check the [video reference](https://rerun.io/docs/reference/video).
///
/// All components except `sample` are typically logged statically once per entity.
/// `sample` is then logged repeatedly for each frame on the timeline.
///
/// TODO(#10422): [archetypes.VideoFrameReference] does not yet work with [archetypes.VideoStream].
///
/// \example archetypes/video_stream_synthetic missing="cpp,rs" title="Live streaming of on-the-fly encoded video" image="https://static.rerun.io/video_stream_synthetic/4dd34da01980afa5604994fa4cce34d7573b0763/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Video")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "VideoStream")]
pub struct VideoStream {
    /// The codec used to encode the video chunks.
    ///
    /// This property is expected to be constant over time and is ideally logged statically once per stream.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub codec: rerun::components::VideoCodec,

    /// Video sample data (also known as "video chunk").
    ///
    /// The current timestamp is used as presentation timestamp (PTS) for all data in this sample.
    /// There is currently no way to log differing decoding timestamps, meaning
    /// that there is no support for B-frames.
    /// See <https://github.com/rerun-io/rerun/issues/10090> for more details.
    // TODO(#10090): See above.
    ///
    /// Rerun chunks containing frames (i.e. bundles of sample data) may arrive out of order,
    /// but may cause the video playback in the Viewer to reset.
    /// It is recommended to have all chunks for a video stream to be ordered temporally order.
    ///
    /// Logging separate videos on the same entity is allowed iff they share the exact same
    /// codec parameters & resolution.
    ///
    /// The samples are expected to be encoded using the `codec` field.
    /// Each video sample must contain enough data for exactly one video frame
    /// (this restriction may be relaxed in the future for some codecs).
    ///
    /// Unless your stream consists entirely of key-frames (in which case you should consider [archetypes.EncodedImage])
    /// never log this component as static data as this means that you loose all information of
    /// previous samples which may be required to decode an image.
    ///
    /// See [components.VideoCodec] for codec specific requirements.
    #[rerun(recommended)]
    pub sample: Option<rerun::components::VideoSample>,

    /// Whether the corresponding [components.VideoSample] contains a keyframe.
    ///
    /// A keyframe (also known as a sync sample or IDR) is a frame from which a decoder can
    /// start decoding the stream with no prior decoder state. See [components.IsKeyframe]
    /// and [components.VideoCodec] for the codec-specific definition.
    ///
    /// This field is optional. It does not change how the stream itself is decoded: it is
    /// metadata that travels with the sample and can be inspected when querying the data
    /// back, for example to locate sync points or build a frame index.
    #[rerun(optional)]
    pub is_keyframe: Option<rerun::components::IsKeyframe>,

    // TODO(#3982): add orientation.
    /// Opacity of the video stream, useful for layering several media.
    ///
    /// Defaults to 1.0 (fully opaque).
    #[rerun(optional)]
    pub opacity: Option<rerun::components::Opacity>,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    /// Defaults to `-15.0`.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,
}
