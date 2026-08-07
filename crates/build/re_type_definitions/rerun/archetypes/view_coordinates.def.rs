// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Sets the default orientation for 3D views at or below this entity.
///
/// The orientation determines the 3D view's eye orientation, navigation, and default grid plane.
/// It does not change logged transforms.
///
/// The three directions are always ordered as [x, y, z] and specify where each positive axis points.
/// For example, [Right, Down, Forward] means that +X points right, +Y points down, and +Z points forward.
///
/// A 3D view uses the value logged at its origin entity or the closest ancestor.
/// [SpatialInformation](https://rerun.io/docs/reference/types/views/spatial3d_view) can override it for an individual view.
///
/// ⚠ [Rerun does not yet support left-handed coordinate systems](https://github.com/rerun-io/rerun/issues/5032).
///
/// \example archetypes/view_coordinates_simple title="Set the default 3D view orientation" image="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Transforms")]
#[docs(view_types = "Spatial3DView")]
#[rerun(state = "unstable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
#[rust(repr = "transparent")]
pub struct ViewCoordinates {
    /// The directions of the [x, y, z] axes.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub xyz: rerun::components::ViewCoordinates,
}
