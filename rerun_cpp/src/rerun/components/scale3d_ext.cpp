namespace rerun::components {
#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct `Scale3D` from x/y/z values.
    Scale3D(float x, float y, float z) : scale{x, y, z} {}

    /// Construct `Scale3D` from x/y/z float pointer.
    explicit Scale3D(const float* xyz) : scale{xyz[0], xyz[1], xyz[2]} {}

    /// Construct a `Scale3D` from a uniform scale factor.
    explicit Scale3D(float uniform_scale) : Scale3D(datatypes::Vec3D{uniform_scale, uniform_scale, uniform_scale}) {}

    /// Explicitly construct a `Scale3D` from a uniform scale factor.
    static Scale3D uniform(float uniform_scale) {
        return Scale3D(uniform_scale);
    }

    /// Explicitly construct a `Scale3D` from a 3D scale factor.
    static Scale3D three_d(datatypes::Vec3D scale) {
        return Scale3D(scale);
    }

    // </CODEGEN_COPY_TO_HEADER>

#endif
} // namespace rerun::components
