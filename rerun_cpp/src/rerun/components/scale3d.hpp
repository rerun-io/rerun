// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/scale3d.fbs".

#pragma once

#include "../datatypes/vec3d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: A 3D scale factor.
    ///
    /// A scale of 1.0 means no scaling.
    /// A scale of 2.0 means doubling the size.
    /// Each component scales along the corresponding axis.
    struct Scale3D {
        rerun::datatypes::Vec3D scale;

      public: // START of extensions from scale3d_ext.cpp:
        /// Construct `Scale3D` from x/y/z values.
        Scale3D(float x, float y, float z) : scale{x, y, z} {}

        /// Construct `Scale3D` from x/y/z float pointer.
        explicit Scale3D(const float* xyz) : scale{xyz[0], xyz[1], xyz[2]} {}

        /// Construct a `Scale3D` from a uniform scale factor.
        explicit Scale3D(float uniform_scale)
            : Scale3D(datatypes::Vec3D{uniform_scale, uniform_scale, uniform_scale}) {}

        /// Explicitly construct a `Scale3D` from a uniform scale factor.
        static Scale3D uniform(float uniform_scale) {
            return Scale3D(uniform_scale);
        }

        /// Explicitly construct a `Scale3D` from a 3D scale factor.
        static Scale3D three_d(datatypes::Vec3D scale) {
            return Scale3D(scale);
        }

        // END of extensions from scale3d_ext.cpp, start of generated code:

      public:
        Scale3D() = default;

        Scale3D(rerun::datatypes::Vec3D scale_) : scale(scale_) {}

        Scale3D& operator=(rerun::datatypes::Vec3D scale_) {
            scale = scale_;
            return *this;
        }

        Scale3D(std::array<float, 3> xyz_) : scale(xyz_) {}

        Scale3D& operator=(std::array<float, 3> xyz_) {
            scale = xyz_;
            return *this;
        }

        /// Cast to the underlying Vec3D datatype
        operator rerun::datatypes::Vec3D() const {
            return scale;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Vec3D) == sizeof(components::Scale3D));

    /// \private
    template <>
    struct Loggable<components::Scale3D> {
        static constexpr const char Name[] = "rerun.components.Scale3D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Vec3D>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::Scale3D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Scale3D* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Vec3D>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Vec3D>::to_arrow(
                    &instances->scale,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
