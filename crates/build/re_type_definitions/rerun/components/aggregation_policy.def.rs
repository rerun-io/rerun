// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Policy for aggregation of multiple scalar plot values.
///
/// This is used for lines in plots when the X axis distance of individual points goes below a single pixel,
/// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
/// (and readability) in such situations as it prevents overdraw.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum AggregationPolicy {
    /// No aggregation.
    Off = 1,

    /// Average all points in the range together.
    ///
    /// This can wash out outliers (spikes).
    Average = 2,

    /// Keep only the maximum values in the range.
    Max = 3,

    /// Keep only the minimum values in the range.
    Min = 4,

    /// Keep both the minimum and maximum values in the range.
    ///
    /// This will yield two aggregated points instead of one, effectively creating a vertical line.
    /// In practice this often leads to a rather ugly zig-zag look.
    // TODO(#4969): output a thicker line instead of zig-zagging.
    MinMax = 5,

    /// Find both the minimum and maximum values in the range, then use the average of those.
    ///
    /// This yields a single point per range, so it does not draw a vertical line per pixel,
    /// while still letting a lone outlier move the plotted value, which averaging all the
    /// points in the range would wash out.
    #[default]
    MinMaxAverage = 6,
}
