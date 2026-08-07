// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A transform between two 3D spaces, i.e. a pose.
///
/// From the point of view of the entity's coordinate system,
/// all components are applied in the inverse order they are listed here.
/// E.g. if both a translation and a mat3x3 transform are present,
/// the 3x3 matrix is applied first, followed by the translation.
///
/// Whenever you log this archetype, the state of the resulting transform relationship is fully reset to the new archetype.
/// This means that if you first log a transform with only a translation, and then log one with only a rotation,
/// it will be resolved to a transform with only a rotation.
/// (This is unlike how we usually apply latest-at semantics on an archetype where we take the latest state of any component independently)
///
/// For transforms that affect only a single entity and do not propagate along the entity tree refer to [archetypes.InstancePoses3D].
///
/// \example archetypes/transform3d_simple title="Variety of 3D transforms" image="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1200w.png"
/// \example archetypes/transform3d_hierarchy title="Transform hierarchy" !api image="https://static.rerun.io/transform_hierarchy/c2a22bff0b5ebfb6cd7742069f096f1de984f7b1/full.png"
/// \example archetypes/transform3d_hierarchy_frames title="Transform hierarchy with explicit frames" !api image="https://static.rerun.io/transform_hierarchy_frames/9ffb0079828f46c22e22ca55737b8a903889b412/full.png"
/// \example archetypes/transform3d_row_updates title="Update a transform over time" image="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1200w.png"
/// \example archetypes/transform3d_column_updates title="Update a transform over time, in a single operation" image="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1200w.png"
/// \example archetypes/transform3d_partial_updates title="Update specific properties of a transform over time" image="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Transforms")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct Transform3D {
    // TODO(#6743): Transforms can't be affected by blueprints which is why all components of this archetype are non-ui editable.
    /// Translation vector.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub translation: Option<rerun::components::Translation3D>,

    /// Rotation via axis + angle.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub rotation_axis_angle: Option<rerun::components::RotationAxisAngle>,

    /// Rotation via quaternion.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub quaternion: Option<rerun::components::RotationQuat>,

    /// Scaling factor.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub scale: Option<rerun::components::Scale3D>,

    /// 3x3 transformation matrix.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub mat3x3: Option<rerun::components::TransformMat3x3>,

    /// Specifies the relation this transform establishes between this entity and its parent.
    ///
    /// Any update to this field will reset all other transform properties that aren't changed in the same log call or `send_columns` row.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub relation: Option<rerun::components::TransformRelation>,

    // --- transform frame
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
}
