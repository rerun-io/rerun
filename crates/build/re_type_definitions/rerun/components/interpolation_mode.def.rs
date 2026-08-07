// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies how values between data points are interpolated in time series.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum InterpolationMode {
    /// Connect data points with straight line segments.
    #[default]
    Linear = 1,

    /// Hold the previous value until the next data point, then jump.
    ///
    /// The step occurs at the end of the interval.
    StepAfter = 2,

    /// Jump to the new value immediately, then hold until the next data point.
    ///
    /// The step occurs at the beginning of the interval.
    StepBefore = 3,

    /// Hold the previous value until the midpoint between data points, then jump to the new value.
    ///
    /// The step occurs at the midpoint of the interval.
    StepMid = 4,
}
