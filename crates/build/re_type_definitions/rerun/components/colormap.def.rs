// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Colormap for mapping scalar values within a given range to a color.
///
/// This provides a number of popular pre-defined colormaps.
/// In the future, the Rerun Viewer will allow users to define their own colormaps,
/// but currently the Viewer is limited to the types defined here.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum Colormap {
    /// A simple black to white gradient.
    ///
    /// This is a sRGB gray gradient which is perceptually uniform.
    Grayscale = 1,

    /// The Inferno colormap from Matplotlib.
    ///
    /// This is a perceptually uniform colormap.
    /// It interpolates from black to red to bright yellow.
    Inferno = 2,

    /// The Magma colormap from Matplotlib.
    ///
    /// This is a perceptually uniform colormap.
    /// It interpolates from black to purple to white.
    Magma = 3,

    /// The Plasma colormap from Matplotlib.
    ///
    /// This is a perceptually uniform colormap.
    /// It interpolates from dark blue to purple to yellow.
    Plasma = 4,

    /// Google's Turbo colormap map.
    ///
    /// This is a perceptually non-uniform rainbow colormap addressing many issues of
    /// more traditional rainbow colormaps like Jet.
    /// It is more perceptually uniform without sharp transitions and is more colorblind-friendly.
    /// Details: <https://research.google/blog/turbo-an-improved-rainbow-colormap-for-visualization/>
    #[default]
    Turbo = 5,

    /// The Viridis colormap from Matplotlib
    ///
    /// This is a perceptually uniform colormap which is robust to color blindness.
    /// It interpolates from dark purple to green to yellow.
    Viridis = 6,

    /// Rasmusgo's Cyan to Yellow colormap
    ///
    /// This is a perceptually uniform colormap which is robust to color blindness.
    /// It is especially suited for visualizing signed values.
    /// It interpolates from cyan to blue to dark gray to brass to yellow.
    CyanToYellow = 7,

    /// The Spectral colormap from Matplotlib.
    ///
    /// This is a diverging colormap, often used to visualize data with a meaningful center point,
    /// where deviations from that center are important to highlight.
    /// It interpolates from red to orange to yellow to green to blue to violet.
    Spectral = 8,

    /// The Twilight colormap from Matplotlib.
    ///
    /// This is a perceptually uniform cyclic colormap from Matplotlib, it is useful for
    /// visualizing periodic or cyclic data.
    ///
    /// It interpolates from white to blue to purple to red to orange and back to white.
    Twilight = 9,

    /// The classic `RViz` "Map" grid-map colormap intended for occupancy-style SLAM grid maps.
    ///
    /// Known values are mapped to a grayscale ramp from white (free) to black (occupied),
    /// unknown values are in a green-blue color. Special / illegal values have highlight colors.
    RvizMap = 10,

    /// The classic `RViz` "Costmap" grid-map colormap for robot navigation cost maps.
    ///
    /// Cost values are mapped to blue to red spectrum, and special cost values
    /// (e.g. lethal obstacles) have highlight colors. Zero values are fully transparent.
    RvizCostmap = 11,

    /// A grid-map colormap for robot navigation cost maps.
    ///
    /// Semantically equivalent to the `RViz` cost map but with a more pleasing color palette.
    ///
    /// Cost values are mapped to a green to yellow spectrum, and special cost values
    /// (e.g. lethal obstacles) have highlight colors. Zero values are fully transparent.
    Costmap = 12,
}
