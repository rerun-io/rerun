namespace rerun::components {
#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct `LeafScale3D` from x/y/z values.
    LeafScale3D(float x, float y, float z) : scale{x, y, z} {}

    /// Construct `LeafScale3D` from x/y/z float pointer.
    explicit LeafScale3D(const float* xyz) : scale{xyz[0], xyz[1], xyz[2]} {}

    /// Construct a `LeafScale3D` from a uniform scale factor.
    explicit LeafScale3D(float uniform_scale) : LeafScale3D(datatypes::Vec3D{uniform_scale, uniform_scale, uniform_scale}) {}

    /// Explicitly construct a `LeafScale3D` from a uniform scale factor.
    static LeafScale3D uniform(float uniform_scale) {
        return LeafScale3D(uniform_scale);
    }

    /// Explicitly construct a `LeafScale3D` from a 3D scale factor.
    static LeafScale3D three_d(datatypes::Vec3D scale) {
        return LeafScale3D(scale);
    }

    // </CODEGEN_COPY_TO_HEADER>

#endif
} // namespace rerun::components
