// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies if a graph has directed or undirected edges.
#[rerun::rerun_type]
#[repr(u8)]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub enum GraphType {
    /// The graph has undirected edges.
    #[default]
    Undirected = 1,

    /// The graph has directed edges.
    Directed = 2,
}
