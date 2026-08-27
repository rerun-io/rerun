// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The controls for the 3D eye in a spatial 3D view.
///
/// This configures the camera through which the 3D scene is viewed.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct EyeControls3D {
    /// The kind of the eye for the spatial 3D view.
    ///
    /// This controls how the eye movement behaves when the user interact with the view.
    /// Defaults to orbital.
    #[rerun(optional)]
    pub kind: Option<rerun::blueprint::components::Eye3DKind>,

    /// The cameras current position.
    #[rerun(optional)]
    pub position: Option<rerun::components::Position3D>,

    /// The position the camera is currently looking at.
    ///
    /// If this is an orbital camera, this also is the center it orbits around.
    ///
    /// By default this is the center of the scene bounds.
    #[rerun(optional)]
    pub look_target: Option<rerun::components::Position3D>,

    /// The up-axis of the eye itself, in world-space.
    ///
    /// Initially, the up-axis of the eye will be the same as the up-axis of the scene (or +Z if
    /// the scene has no up axis defined).
    #[rerun(optional)]
    pub eye_up: Option<rerun::components::Vector3D>,

    /// Translation speed of the eye in the view (when using WASDQE keys to move in the 3D scene).
    ///
    /// The default depends on the control kind.
    /// For orbit cameras it is derived from the distance to the orbit center.
    /// For first person cameras it is derived from the scene size.
    #[rerun(optional)]
    pub speed: Option<rerun::components::LinearSpeed>,

    /// Currently tracked entity.
    ///
    /// If this is a camera, it takes over the camera pose, otherwise follows the entity.
    #[rerun(optional)]
    pub tracking_entity: Option<rerun::components::EntityPath>,

    /// What speed, if any, the camera should spin around the eye-up axis.
    ///
    /// Defaults to zero, meaning no spinning.
    #[rerun(optional)]
    pub spin_speed: Option<rerun::blueprint::components::AngularSpeed>,
}
