// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/rotation_axis_angle.fbs"

#pragma once

#include "../datatypes/angle.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>

namespace rerun {
    namespace datatypes {
        /// 3D rotation represented by a rotation around a given axis.
        struct RotationAxisAngle {
            /// Axis to rotate around.
            ///
            /// This is not required to be normalized.
            /// If normalization fails (typically because the vector is length zero), the rotation
            /// is silently ignored.
            rerun::datatypes::Vec3D axis;

            /// How much to rotate around the axis.
            rerun::datatypes::Angle angle;

          public:
            // Extensions to generated type defined in 'rotation_axis_angle_ext.cpp'

            RotationAxisAngle(const Vec3D& _axis, const Angle& _angle)
                : axis(_axis), angle(_angle) {}

          public:
            RotationAxisAngle() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::StructBuilder* builder, const RotationAxisAngle* elements,
                size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
