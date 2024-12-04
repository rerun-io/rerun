// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/rotation_quat.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../datatypes/quaternion.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: A 3D rotation expressed as a quaternion that doesn't propagate in the transform hierarchy.
    ///
    /// Note: although the x,y,z,w components of the quaternion will be passed through to the
    /// datastore as provided, when used in the Viewer, quaternions will always be normalized.
    struct PoseRotationQuat {
        rerun::datatypes::Quaternion quaternion;

      public:
        PoseRotationQuat() = default;

        PoseRotationQuat(rerun::datatypes::Quaternion quaternion_) : quaternion(quaternion_) {}

        PoseRotationQuat& operator=(rerun::datatypes::Quaternion quaternion_) {
            quaternion = quaternion_;
            return *this;
        }

        /// Cast to the underlying Quaternion datatype
        operator rerun::datatypes::Quaternion() const {
            return quaternion;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Quaternion) == sizeof(components::PoseRotationQuat));

    /// \private
    template <>
    struct Loggable<components::PoseRotationQuat> {
        static constexpr ComponentDescriptor Descriptor = "rerun.components.PoseRotationQuat";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Quaternion>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::PoseRotationQuat` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::PoseRotationQuat* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Quaternion>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Quaternion>::to_arrow(
                    &instances->quaternion,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
