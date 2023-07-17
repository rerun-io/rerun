// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/rotation3d.fbs"

#pragma once

#include <cstdint>

#include "../datatypes/quaternion.hpp"
#include "../datatypes/rotation_axis_angle.hpp"

namespace rr {
    namespace datatypes {
        namespace detail {
            enum class Rotation3DTag {
                NONE = 0, // Makes it possible to implement move semantics
                Quaternion,
                AxisAngle,
            };

            union Rotation3DData {
                /// Rotation defined by a quaternion.
                rr::datatypes::Quaternion quaternion;

                /// Rotation defined with an axis and an angle.
                rr::datatypes::RotationAxisAngle axis_angle;

                Rotation3DData() {}

                ~Rotation3DData() {}
            };
        } // namespace detail

        /// A 3D rotation.
        struct Rotation3D {
          private:
            detail::Rotation3DTag _tag;
            detail::Rotation3DData _data;

            Rotation3D() : _tag(detail::Rotation3DTag::NONE) {}

          public:
        };
    } // namespace datatypes
} // namespace rr
