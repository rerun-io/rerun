// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configures what range of each timeline is shown on a view.
///
/// Whenever no visual time range applies, queries are done with "latest-at" semantics.
/// This means that the view will, starting from the time cursor position,
/// query the latest data available for each component type.
///
/// The default visual time range depends on the type of view this property applies to:
/// - For time series views, the default is to show the entire timeline.
/// - For any other view, the default is to apply latest-at semantics.
#[rerun::rerun_type]
#[python(aliases = "datatypes.VisibleTimeRangeLike | Sequence[datatypes.VisibleTimeRangeLike]")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct VisibleTimeRanges {
    /// The time ranges to show for each timeline unless specified otherwise on a per-entity basis.
    ///
    /// If a timeline is specified more than once, the first entry will be used.
    #[rerun(required)]
    pub ranges: Vec<rerun::blueprint::components::VisibleTimeRange>,
}
