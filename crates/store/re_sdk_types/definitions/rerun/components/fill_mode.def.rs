// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How a geometric shape is drawn and colored.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum FillMode {
    /// Lines are drawn around the parts of the shape which directly correspond to the logged data.
    ///
    /// Examples of what this means:
    ///
    /// * An [archetypes.Ellipsoids3D] will draw three axis-aligned ellipses that are cross-sections
    ///   of each ellipsoid, each of which displays two out of three of the sizes of the ellipsoid.
    /// * For [archetypes.Boxes3D], it is the edges of the box, identical to [components.FillMode.DenseWireframe].
    MajorWireframe = 1,

    /// Many lines are drawn to represent the surface of the shape in a see-through fashion.
    ///
    /// Examples of what this means:
    ///
    /// * An [archetypes.Ellipsoids3D] will draw a wireframe triangle mesh that approximates each
    ///   ellipsoid.
    /// * For [archetypes.Boxes3D], it is the edges of the box, identical to [components.FillMode.MajorWireframe].
    DenseWireframe = 2,

    /// The surface of the shape is filled in with a solid color. No lines are drawn.
    Solid = 3,

    /// The surface of the shape is filled in with a transparent color, with major wireframe lines on top.
    ///
    /// This gives a good default appearance that shows both the shape's surface and its structure.
    #[default]
    TransparentFillMajorWireframe = 4,
}
