// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The top-level description of the viewport.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct ViewportBlueprint {
    /// The layout of the views
    #[rerun(optional)]
    pub root_container: Option<rerun::blueprint::components::RootContainer>,

    /// Show one tab as maximized?
    #[rerun(optional)]
    pub maximized: Option<rerun::blueprint::components::ViewMaximized>,

    // TODO(andreas): This is to be removed in the future, all new views without an explicit container
    // should always insert themselves using a heuristic.
    /// Whether the viewport layout is determined automatically.
    ///
    /// If `true`, the container layout will be reset whenever a new view is added or removed.
    /// This defaults to `false` and is automatically set to `false` when there is user determined layout.
    #[rerun(optional)]
    pub auto_layout: Option<rerun::blueprint::components::AutoLayout>,

    // TODO(jleibs): This should come with an optional container id that specifies where to insert new views.
    /// Whether or not views should be created automatically.
    ///
    /// If `true`, the viewer will only add views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
    /// and which aren't deemed redundant to existing views.
    /// This defaults to `false` and is automatically set to `false` when the user adds views manually in the viewer.
    #[rerun(optional)]
    pub auto_views: Option<rerun::blueprint::components::AutoViews>,

    /// Hashes of all recommended views the viewer has already added and that should not be added again.
    ///
    /// This is an internal field and should not be set usually.
    /// If you want the viewer from stopping to add views, you should set `auto_views` to `false`.
    ///
    /// The viewer uses this to determine whether it should keep adding views.
    #[rerun(optional)]
    pub past_viewer_recommendations:
        Option<Vec<rerun::blueprint::components::ViewerRecommendationHash>>,
}
