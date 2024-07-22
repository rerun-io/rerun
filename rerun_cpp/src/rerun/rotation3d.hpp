#pragma once

#include <optional>

#include "components/rotation_axis_angle.hpp"
#include "components/rotation_quat.hpp"
#include "datatypes/quaternion.hpp"
#include "datatypes/rotation_axis_angle.hpp"

namespace rerun {
    /// Utility for representing a single 3D rotation, agnostic to the underlying representation.
    ///
    /// This is not a component, but a utility for building `rerun::Transform3D`.
    struct Rotation3D {
        std::optional<rerun::components::RotationAxisAngle> axis_angle;
        std::optional<rerun::components::RotationQuat> quaternion;

      public:
        Rotation3D() : axis_angle(std::nullopt), quaternion(std::nullopt) {}

        /// Construct a `Rotation3D` from a rotation axis and angle component.
        Rotation3D(rerun::components::RotationAxisAngle axis_angle_) : axis_angle(axis_angle_) {}

        /// Construct a `Rotation3D` from a quaternion component.
        Rotation3D(rerun::components::RotationQuat quaternion_) : quaternion(quaternion_) {}

        /// Construct a `Rotation3D` from a rotation axis and angle datatype.
        Rotation3D(rerun::datatypes::RotationAxisAngle axis_angle_) : axis_angle(axis_angle_) {}

        /// Construct a `Rotation3D` from a quaternion datatype.
        Rotation3D(rerun::datatypes::Quaternion quaternion_) : quaternion(quaternion_) {}
    };
} // namespace rerun
