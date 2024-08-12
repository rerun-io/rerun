namespace rerun::components {
#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct `PoseScale3D` from x/y/z values.
    PoseScale3D(float x, float y, float z) : scale{x, y, z} {}

    /// Construct `PoseScale3D` from x/y/z float pointer.
    explicit PoseScale3D(const float* xyz) : scale{xyz[0], xyz[1], xyz[2]} {}

    /// Construct a `PoseScale3D` from a uniform scale factor.
    explicit PoseScale3D(float uniform_scale) : PoseScale3D(datatypes::Vec3D{uniform_scale, uniform_scale, uniform_scale}) {}

    /// Explicitly construct a `PoseScale3D` from a uniform scale factor.
    static PoseScale3D uniform(float uniform_scale) {
        return PoseScale3D(uniform_scale);
    }

    /// Explicitly construct a `PoseScale3D` from a 3D scale factor.
    static PoseScale3D three_d(datatypes::Vec3D scale) {
        return PoseScale3D(scale);
    }

    // </CODEGEN_COPY_TO_HEADER>

#endif
} // namespace rerun::components
