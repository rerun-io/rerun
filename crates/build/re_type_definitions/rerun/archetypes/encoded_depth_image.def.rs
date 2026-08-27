// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A depth image encoded with a codec (e.g. RVL or PNG).
///
/// Rerun also supports uncompressed depth images with the [`archetypes.DepthImage`](https://rerun.io/docs/reference/types/archetypes/depth_image).
///
/// \example archetypes/encoded_depth_image title="Encoded depth image" image="https://static.rerun.io/encoded_depth_image/d8180f8167278f9601808c360ba52eafaab52839/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Image & tensor")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "EncodedDepthImage")]
#[rust(derive(PartialEq))]
pub struct EncodedDepthImage {
    /// The encoded depth payload.
    ///
    /// Supported are:
    /// * single channel PNG
    /// * RVL with ROS2 metadata (for details see <https://github.com/ros-perception/image_transport_plugins/tree/jazzy>)
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub blob: rerun::components::Blob,

    /// Media type of the blob, e.g.:
    ///
    ///  * `application/rvl` (RVL-compressed 16-bit)
    ///  * `image/png`
    #[rerun(recommended)]
    pub media_type: Option<rerun::components::MediaType>,

    /// Conversion from native units to meters (e.g. `0.001` for millimeters).
    ///
    /// If omitted, the Viewer defaults to `1.0` for floating-point depth formats and `1000.0` for integer formats (millimeters).
    #[rerun(recommended)]
    pub meter: Option<rerun::components::DepthMeter>,

    /// Optional colormap for visualization of decoded depth.
    #[rerun(optional)]
    pub colormap: Option<rerun::components::Colormap>,

    /// Optional visualization range for depth values.
    #[rerun(optional)]
    pub depth_range: Option<rerun::components::ValueRange>,

    /// Optional point fill ratio for point-cloud projection.
    #[rerun(optional)]
    pub point_fill_ratio: Option<rerun::components::FillRatio>,

    /// Optional 2D draw order.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,

    /// Optional filter used when a texel is magnified (displayed larger than a screen pixel) in 2D views.
    ///
    /// The filter is applied to the scalar values *before* they are mapped to color via the colormap.
    ///
    /// Has no effect in 3D views.
    #[rerun(optional)]
    pub magnification_filter: Option<rerun::components::MagnificationFilter>,
}
