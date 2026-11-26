// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the rulers along the edges of a 2D view.
///
/// The rulers show the scene coordinates along two edges of the view,
/// similar to the axes of a plot.
/// The horizontal ruler runs along the edge where Y is lowest,
/// and the vertical ruler along the edge where X is lowest.
/// With X pointing right and Y pointing down, these are the top and left edge.
#[rerun::rerun_type]
#[python(aliases = "encodings.BoolLike")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct Rulers {
    /// Whether the rulers are visible.
    ///
    /// Defaults to false.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,

    /// Name shown at the end of the horizontal ruler where X is highest.
    ///
    /// Empty by default. An empty name is not shown.
    #[rerun(optional)]
    pub name_x: Option<rerun::components::Name>,

    /// Name shown at the end of the vertical ruler where Y is highest.
    ///
    /// Empty by default. An empty name is not shown.
    #[rerun(optional)]
    pub name_y: Option<rerun::components::Name>,
}
