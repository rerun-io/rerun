// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A depth image, i.e. as captured by a depth camera.
///
/// Each pixel corresponds to a depth value in units specified by [components.DepthMeter].
///
/// \cpp Since the underlying `rerun::datatypes::ImageBuffer` uses `rerun::Collection` internally,
/// \cpp data can be passed in without a copy from raw pointers or by reference from `std::vector`/`std::array`/c-arrays.
/// \cpp If needed, this "borrow-behavior" can be extended by defining your own `rerun::CollectionAdapter`.
///
/// \example archetypes/depth_image_simple !api title="Simple example" image="https://static.rerun.io/depth_image_simple/77a6fa4f938a742bdc7c5350f668c4f31eed4d01/1200w.png"
/// \example archetypes/depth_image_3d title="Depth to 3D example" image="https://static.rerun.io/depth_image_3d/924e9d4d6a39d63d4fdece82582855fdaa62d15e/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Image & tensor")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "DepthImage")]
#[rust(derive(PartialEq))]
pub struct DepthImage {
    /// The raw depth image data.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub buffer: rerun::components::ImageBuffer,

    /// The format of the image.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub format: rerun::components::ImageFormat,

    /// An optional floating point value that specifies how long a meter is in the native depth units.
    ///
    /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
    /// and a range of up to ~65 meters (2^16 / 1000).
    ///
    /// If omitted, the Viewer defaults to `1.0` for floating-point depth formats and `1000.0` for integer formats (millimeters).
    ///
    /// Note that the only effect on 2D views is the physical depth values shown when hovering the image.
    /// In 3D views on the other hand, this affects where the points of the point cloud are placed.
    #[rerun(optional)]
    pub meter: Option<rerun::components::DepthMeter>,

    /// Colormap to use for rendering the depth image.
    ///
    /// If not set, the depth image will be rendered using the Turbo colormap.
    #[rerun(optional)]
    pub colormap: Option<rerun::components::Colormap>,

    /// The expected range of depth values.
    ///
    /// This is typically the expected range of valid values.
    /// Everything outside of the range is clamped to the range for the purpose of colormpaping.
    /// Note that point clouds generated from this image will still display all points, regardless of this range.
    ///
    /// If not specified, the range will be automatically estimated from the data.
    /// Note that the Viewer may try to guess a wider range than the minimum/maximum of values
    /// in the contents of the depth image.
    /// E.g. if all values are positive, some bigger than 1.0 and all smaller than 255.0,
    /// the Viewer will guess that the data likely came from an 8bit image, thus assuming a range of 0-255.
    #[rerun(optional)]
    pub depth_range: Option<rerun::components::ValueRange>,

    /// Scale the radii of the points in the point cloud generated from this image.
    ///
    /// A fill ratio of 1.0 (the default) means that each point is as big as to touch the center of its neighbor
    /// if it is at the same depth, leaving no gaps.
    /// A fill ratio of 0.5 means that each point touches the edge of its neighbor if it has the same depth.
    ///
    /// TODO(#6744): This applies only to 3D views!
    #[rerun(optional)]
    pub point_fill_ratio: Option<rerun::components::FillRatio>,

    /// An optional floating point value that specifies the 2D drawing order, used only if the depth image is shown as a 2D image.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    /// Defaults to `-20.0`.
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
