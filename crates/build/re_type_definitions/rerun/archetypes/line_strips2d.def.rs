// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 2D line strips with positions and optional colors, radii, labels, etc.
///
/// \example archetypes/line_strips2d_simple !api image="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/1200w.png"
/// \example archetypes/line_strips2d_segments_simple !api image="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/1200w.png"
/// \example archetypes/line_strips2d_batch image="https://static.rerun.io/line_strip2d_batch/c6f4062bcf510462d298a5dfe9fdbe87c754acee/1200w.png"
/// \example archetypes/line_strips2d_ui_radius title="Lines with scene & UI radius each" image="https://static.rerun.io/line_strip2d_ui_radius/d3d7d5bd36278564ee37e2ed6932963ec16f5131/1200w.png
#[rerun::rerun_type]
#[docs(category = "Spatial 2D")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Lines2D")]
#[rust(derive(PartialEq))]
pub struct LineStrips2D {
    /// All the actual 2D line strips that make up the batch.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub strips: Vec<rerun::components::LineStrip2D>,

    /// Optional radii for the line strips.
    #[rerun(recommended)]
    pub radii: Option<Vec<rerun::components::Radius>>,

    /// Optional colors for the line strips.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional text labels for the line strips.
    ///
    /// If there's a single label present, it will be placed at the center of the entity.
    /// Otherwise, each instance will have its own label.
    #[rerun(optional)]
    pub labels: Option<Vec<rerun::components::Text>>,

    /// Whether the text labels should be shown.
    ///
    /// If not set, labels will automatically appear when there is exactly one label for this entity
    /// or the number of instances on this entity is under a certain threshold.
    #[rerun(optional)]
    pub show_labels: Option<rerun::components::ShowLabels>,

    /// An optional floating point value that specifies the 2D drawing order of each line strip.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    /// Defaults to `20.0`.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,

    /// Optional [components.ClassId]s for the lines.
    ///
    /// The [components.ClassId] provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
}
