// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An edge in a graph connecting two nodes.
#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct GraphEdge {
    pub edge: rerun::datatypes::Utf8Pair,
}
