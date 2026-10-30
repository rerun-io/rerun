// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether a record is flagged, e.g. for review.
///
/// The Viewer shows it as a flag button.
/// Table card layouts show the first flag field in the card header.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct Flag {
    pub flagged: rerun::encodings::Bool,
}
