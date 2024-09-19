// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/rotation_axis_angle.fbs".

#pragma once

#include "../datatypes/rotation_axis_angle.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: 3D rotation represented by a rotation around a given axis that doesn't propagate in the transform hierarchy.
    struct PoseRotationAxisAngle {
        rerun::datatypes::RotationAxisAngle rotation;

      public:
        PoseRotationAxisAngle() = default;

        PoseRotationAxisAngle(rerun::datatypes::RotationAxisAngle rotation_)
            : rotation(rotation_) {}

        PoseRotationAxisAngle& operator=(rerun::datatypes::RotationAxisAngle rotation_) {
            rotation = rotation_;
            return *this;
        }

        /// Cast to the underlying RotationAxisAngle datatype
        operator rerun::datatypes::RotationAxisAngle() const {
            return rotation;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::RotationAxisAngle) == sizeof(components::PoseRotationAxisAngle)
    );

    /// \private
    template <>
    struct Loggable<components::PoseRotationAxisAngle> {
        static constexpr const char Name[] = "rerun.components.PoseRotationAxisAngle";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::RotationAxisAngle>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::PoseRotationAxisAngle` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::PoseRotationAxisAngle* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::RotationAxisAngle>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::RotationAxisAngle>::to_arrow(
                    &instances->rotation,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
