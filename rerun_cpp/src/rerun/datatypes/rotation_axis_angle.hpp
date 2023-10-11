// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/rotation_axis_angle.fbs".

#pragma once

#include "../result.hpp"
#include "angle.hpp"
#include "vec3d.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// **Datatype**: 3D rotation represented by a rotation around a given axis.
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
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const RotationAxisAngle* elements,
                size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
