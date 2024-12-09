// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/position3d.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../datatypes/vec3d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: A position in 3D space.
    struct Position3D {
        rerun::datatypes::Vec3D xyz;

      public: // START of extensions from position3d_ext.cpp:
        /// Construct Position3D from x/y/z values.
        Position3D(float x, float y, float z) : xyz{x, y, z} {}

        float x() const {
            return xyz.x();
        }

        float y() const {
            return xyz.y();
        }

        float z() const {
            return xyz.z();
        }

        // END of extensions from position3d_ext.cpp, start of generated code:

      public:
        Position3D() = default;

        Position3D(rerun::datatypes::Vec3D xyz_) : xyz(xyz_) {}

        Position3D& operator=(rerun::datatypes::Vec3D xyz_) {
            xyz = xyz_;
            return *this;
        }

        Position3D(std::array<float, 3> xyz_) : xyz(xyz_) {}

        Position3D& operator=(std::array<float, 3> xyz_) {
            xyz = xyz_;
            return *this;
        }

        /// Cast to the underlying Vec3D datatype
        operator rerun::datatypes::Vec3D() const {
            return xyz;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Vec3D) == sizeof(components::Position3D));

    /// \private
    template <>
    struct Loggable<components::Position3D> {
        static constexpr ComponentDescriptor Descriptor = "rerun.components.Position3D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Vec3D>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::Position3D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Position3D* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Vec3D>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Vec3D>::to_arrow(&instances->xyz, num_instances);
            }
        }
    };
} // namespace rerun
