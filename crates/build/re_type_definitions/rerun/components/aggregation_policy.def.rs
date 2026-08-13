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
    Average = 2,

    /// Keep only the maximum values in the range.
    Max = 3,

    /// Keep only the minimum values in the range.
    Min = 4,

    /// Keep both the minimum and maximum values in the range.
    ///
    /// This will yield two aggregated points instead of one, effectively creating a vertical line.
    #[default]
    MinMax = 5,

    /// Find both the minimum and maximum values in the range, then use the average of those.
    MinMaxAverage = 6,
}
