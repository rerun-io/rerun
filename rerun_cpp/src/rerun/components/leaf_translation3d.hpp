// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/translation3d.fbs".

#pragma once

#include "../datatypes/vec3d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: A translation vector in 3D space that doesn't propagate in the transform hierarchy.
    struct LeafTranslation3D {
        rerun::datatypes::Vec3D vector;

      public: // START of extensions from leaf_translation3d_ext.cpp:
        /// Construct `LeafTranslation3D` from x/y/z values.
        LeafTranslation3D(float x, float y, float z) : vector{x, y, z} {}

        /// Construct `LeafTranslation3D` from x/y/z float pointer.
        explicit LeafTranslation3D(const float* xyz) : vector{xyz[0], xyz[1], xyz[2]} {}

        float x() const {
            return vector.x();
        }

        float y() const {
            return vector.y();
        }

        float z() const {
            return vector.z();
        }

        // END of extensions from leaf_translation3d_ext.cpp, start of generated code:

      public:
        LeafTranslation3D() = default;

        LeafTranslation3D(rerun::datatypes::Vec3D vector_) : vector(vector_) {}

        LeafTranslation3D& operator=(rerun::datatypes::Vec3D vector_) {
            vector = vector_;
            return *this;
        }

        LeafTranslation3D(std::array<float, 3> xyz_) : vector(xyz_) {}

        LeafTranslation3D& operator=(std::array<float, 3> xyz_) {
            vector = xyz_;
            return *this;
        }

        /// Cast to the underlying Vec3D datatype
        operator rerun::datatypes::Vec3D() const {
            return vector;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Vec3D) == sizeof(components::LeafTranslation3D));

    /// \private
    template <>
    struct Loggable<components::LeafTranslation3D> {
        static constexpr const char Name[] = "rerun.components.LeafTranslation3D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Vec3D>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::LeafTranslation3D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::LeafTranslation3D* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Vec3D>::to_arrow(&instances->vector, num_instances);
        }
    };
} // namespace rerun
