// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The description of a single view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ViewBlueprint {
    /// The class of the view.
    #[rerun(required)]
    pub class_identifier: rerun::blueprint::components::ViewClass,

    /// The name of the view.
    #[rerun(optional)]
    pub display_name: Option<rerun::components::Name>,

    /// The "anchor point" of this view.
    ///
    /// In other words, the coordinate frame at this entity becomes the reference frame of the view.
    ///
    /// Defaults to the root path '/' if not specified.
    ///
    /// The transform at this path forms the reference point for all scene->world transforms in this view.
    /// I.e. the position of this entity path in space forms the origin of the coordinate system in this view.
    /// Furthermore, this is the primary indicator for heuristics on what entities we show in this view.
    #[rerun(optional)]
    pub space_origin: Option<rerun::blueprint::components::ViewOrigin>,

    /// Whether this view is visible.
    ///
    /// Defaults to true if not specified.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,
}
