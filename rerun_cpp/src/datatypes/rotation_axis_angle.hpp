// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/rotation_axis_angle.fbs"

#pragma once

#include "../datatypes/angle.hpp"
#include "../datatypes/vec3d.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
}

namespace rr {
    namespace datatypes {
        /// 3D rotation represented by a rotation around a given axis.
        struct RotationAxisAngle {
            /// Axis to rotate around.
            ///
            /// This is not required to be normalized.
            /// If normalization fails (typically because the vector is length zero), the rotation
            /// is silently ignored.
            rr::datatypes::Vec3D axis;

            /// How much to rotate around the axis.
            rr::datatypes::Angle angle;

          public:
            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();
        };
    } // namespace datatypes
} // namespace rr
