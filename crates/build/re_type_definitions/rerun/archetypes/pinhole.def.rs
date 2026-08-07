// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Camera perspective projection (a.k.a. intrinsics).
///
/// If [archetypes.Transform3D] is logged for the same child/parent relationship (e.g. for the camera extrinsics), it takes precedence over [archetypes.Pinhole].
///
/// If you use named transform frames via the `child_frame` and `parent_frame` fields, you don't have to use [archetypes.CoordinateFrame]
/// as it is the case with other visualizations: for any entity with an [archetypes.Pinhole] the viewer will always visualize it
/// directly without needing a [archetypes.CoordinateFrame] to refer to the pinhole's child/parent frame.
///
/// \example archetypes/pinhole_simple title="Simple pinhole camera" image="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/1200w.png"
/// \example archetypes/pinhole_perspective title="Perspective pinhole camera" image="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/1200w.png"
/// \example archetypes/pinhole_projections title="Projection setup with blueprints" !api image="https://static.rerun.io/pinhole-projections/ceb1b4124e111b5d0a786dd48909a1cbb52eca4c/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Transforms")]
#[docs(view_types = "Spatial2DView, Spatial3DView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Cameras")]
#[rust(derive(PartialEq))]
pub struct Pinhole {
    // TODO(#6743): Transforms can't be affected by blueprints which is why most components here are non-ui editable.
    // Note that pure styling components like `color` and `line_width` are still editable as they don't affect the transform itself, but the pinhole projection components are non-editable.

    // --- Camera parameters ---
    /// Camera projection, from image coordinates to view coordinates.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub image_from_camera: rerun::components::PinholeProjection,

    /// Pixel resolution (usually integers) of child image space. Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    ///
    /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(recommended)]
    pub resolution: Option<rerun::components::Resolution>,

    // --- Other ---
    /// Sets the camera orientation convention.
    ///
    /// All common values are available as constants on the [components.ViewCoordinates] class.
    ///
    /// The default is `ViewCoordinates::RDF`: +X is right, +Y is down, and +Z is forward.
    /// This makes the camera frustum point along +Z in the parent space, with its up direction along -Y.
    ///
    /// The camera frustum points along the axis set to `F`, or opposite the axis set to `B`.
    /// When logging a depth image under this entity, this is the direction in which the point cloud is projected.
    ///
    /// The frustum's up direction is the axis set to `U`, or opposite the axis set to `D`.
    /// This matches the -Y direction of pixel space, where all images use RDF coordinates.
    ///
    /// The frustum's right direction is the axis set to `R`, or opposite the axis set to `L`.
    /// This matches the +X direction of pixel space.
    ///
    /// Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).
    ///
    /// `image_from_camera` is always defined to project along +Z in camera coordinates.
    /// `camera_xyz` reorients that projection to the forward axis of the pinhole entity.
    // This is excluded from the atomic set because camera orientation may also be read from the `ViewCoordinates` descriptor.
    // Should it be reset when other transform properties are changed?
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub camera_xyz: Option<rerun::components::ViewCoordinates>,

    // --- Topology ---
    /// The child frame this transform transforms from.
    ///
    /// The entity at which the transform relationship of any given child frame is specified mustn't change over time, but is allowed to be different for static time.
    /// E.g. if you specified the child frame `"robot_arm"` on an entity named `"my_transforms"`, you may not log transforms
    /// with the child frame `"robot_arm"` on any other entity than `"my_transforms"` unless one of them was logged with static time.
    ///
    /// If not specified, this is set to the implicit transform frame of the current entity path.
    /// This means that if a [archetypes.Transform3D] is set on an entity called `/my/entity/path` then this will default to `tf#/my/entity/path`.
    ///
    /// To set the frame an entity is part of see [archetypes.CoordinateFrame].
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub child_frame: Option<rerun::components::TransformFrameId>,

    /// The parent frame this transform transforms into.
    ///
    /// If not specified, this is set to the implicit transform frame of the current entity path's parent.
    /// This means that if a [archetypes.Transform3D] is set on an entity called `/my/entity/path` then this will default to `tf#/my/entity`.
    ///
    /// To set the frame an entity is part of see [archetypes.CoordinateFrame].
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub parent_frame: Option<rerun::components::TransformFrameId>,

    // --- Visualization in 3D ---
    /// The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
    ///
    /// This is only used for visualization purposes, and does not affect the projection itself.
    #[rerun(optional)]
    pub image_plane_distance: Option<rerun::components::ImagePlaneDistance>,

    /// Color of the camera wireframe.
    #[rerun(optional)]
    pub color: Option<rerun::components::Color>,

    /// Width of the camera wireframe lines.
    #[rerun(optional)]
    pub line_width: Option<rerun::components::Radius>,
}
