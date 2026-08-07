// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D line strips with positions and optional colors, radii, labels, etc.
///
/// \example archetypes/line_strips3d_simple !api title="Simple example" image="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/1200w.png"
/// \example archetypes/line_strips3d_segments_simple !api title="Many individual segments" image="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/1200w.png"
/// \example archetypes/line_strips3d_batch title="Many strips" image="https://static.rerun.io/line_strip3d_batch/15e8ff18a6c95a3191acb0eae6eb04adea3b4874/1200w.png"
/// \example archetypes/line_strips3d_ui_radius title="Lines with scene & UI radius each" image="https://static.rerun.io/line_strip3d_ui_radius/36b98f47e45747b5a3601511ff39b8d74c61d120/1200w.png"
/// \example archetypes/line_strips3d_time_window missing="cpp,rs" title="Time-windowed trails (e.g. Trajectories)" image="https://static.rerun.io/line_strips3d_time_window/999f92d8f7f09b77e8307e6bbcaad652cf2f2c44/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Lines3D")]
#[rust(derive(PartialEq))]
pub struct LineStrips3D {
    /// All the actual 3D line strips that make up the batch.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub strips: Vec<rerun::components::LineStrip3D>,

    /// Optional radii for the line strips.
    #[rerun(recommended)]
    pub radii: Option<Vec<rerun::components::Radius>>,

    /// Optional colors for the line strips.
    ///
    /// The alpha channel is ignored.
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

    /// Optional [components.ClassId]s for the lines.
    ///
    /// The [components.ClassId] provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
}
