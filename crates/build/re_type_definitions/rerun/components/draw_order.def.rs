// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Draw order of 2D elements. Higher values are drawn on top of lower values.
///
/// An entity can have only a single draw order component.
/// Within an entity draw order is governed by the order of the components.
///
/// Draw order for entities with the same draw order is generally undefined.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float32]")]
#[rust(derive(Copy))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct DrawOrder {
    pub value: rerun::datatypes::Float32,
}
