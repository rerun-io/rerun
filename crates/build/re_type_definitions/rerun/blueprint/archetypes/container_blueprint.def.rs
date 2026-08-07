// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The description of a container.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ContainerBlueprint {
    /// The class of the view.
    #[rerun(required)]
    pub container_kind: rerun::blueprint::components::ContainerKind,

    /// The name of the container.
    #[rerun(optional)]
    pub display_name: Option<rerun::components::Name>,

    /// `ContainerId`s or `ViewId`s that are children of this container.
    #[rerun(optional)]
    pub contents: Option<Vec<rerun::blueprint::components::IncludedContent>>,

    /// The layout shares of each column in the container.
    ///
    /// For [components.ContainerKind.Horizontal] containers, the length of this list should always match the number of contents.
    ///
    /// Ignored for [components.ContainerKind.Vertical] containers.
    #[rerun(optional)]
    pub col_shares: Option<Vec<rerun::blueprint::components::ColumnShare>>,

    /// The layout shares of each row of the container.
    ///
    /// For [components.ContainerKind.Vertical] containers, the length of this list should always match the number of contents.
    ///
    /// Ignored for [components.ContainerKind.Horizontal] containers.
    #[rerun(optional)]
    pub row_shares: Option<Vec<rerun::blueprint::components::RowShare>>,

    /// Which tab is active.
    ///
    /// Only applies to `Tabs` containers.
    #[rerun(optional)]
    pub active_tab: Option<rerun::blueprint::components::ActiveTab>,

    /// Whether this container is visible.
    ///
    /// Defaults to true if not specified.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,

    /// How many columns this grid should have.
    ///
    /// If unset, the grid layout will be auto.
    ///
    /// Ignored for [components.ContainerKind.Horizontal]/[components.ContainerKind.Vertical] containers.
    #[rerun(optional)]
    pub grid_columns: Option<rerun::blueprint::components::GridColumns>,
}
