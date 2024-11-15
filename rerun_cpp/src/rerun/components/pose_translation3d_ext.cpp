namespace rerun::components {
#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct `PoseTranslation3D` from x/y/z values.
    PoseTranslation3D(float x, float y, float z) : vector{x, y, z} {}

    /// Construct `PoseTranslation3D` from x/y/z float pointer.
    explicit PoseTranslation3D(const float* xyz) : vector{xyz[0], xyz[1], xyz[2]} {}

    float x() const {
        return vector.x();
    }

    float y() const {
        return vector.y();
    }

    float z() const {
        return vector.z();
    }

    // </CODEGEN_COPY_TO_HEADER>
#endif
} // namespace rerun::components
