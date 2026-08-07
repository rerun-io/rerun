// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Hash of a viewer recommendation.
///
/// The formation of this hash is considered an internal implementation detail of the viewer.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ViewerRecommendationHash {
    pub value: rerun::datatypes::UInt64,
}
