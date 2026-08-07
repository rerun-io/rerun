// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A color multiplier, usually applied to a whole entity, e.g. a mesh.
#[rerun::rerun_type]
#[rust(derive(
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::Pod,
    bytemuck::Zeroable
))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct AlbedoFactor {
    pub albedo_factor: rerun::datatypes::Rgba32,
}
