// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies the coordinate frame for an entity.
///
/// If not specified, the coordinate frame uses an implicit frame derived from the entity path.
/// The implicit frame's name is `tf#/your/entity/path` and has an identity transform connection to its parent path.
///
/// To learn more about transforms see [Spaces & Transforms](https://rerun.io/docs/concepts/spaces-and-transforms) in the reference.
///
/// \example archetypes/coordinate_frame_builtin_frames title="Change coordinate frame to different built-in frames" image="https://static.rerun.io/coordinate_frame_builtin_frame/71f941f35cf73c299c6ea7fbc4487a140db8e8f8/1200w.png"
/// \example archetypes/transform3d_hierarchy_frames title="Transform hierarchy with explicit frames" !api image="https://static.rerun.io/transform_hierarchy_frames/9ffb0079828f46c22e22ca55737b8a903889b412/full.png"
#[rerun::rerun_type]
#[docs(category = "Transforms")]
#[docs(view_types = "Spatial3DView, Spatial2DView")]
#[rerun(state = "stable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct CoordinateFrame {
    /// The coordinate frame to use for the current entity.
    ///
    /// Note that empty strings are not valid transform frame IDs.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub frame: rerun::components::TransformFrameId,
}
