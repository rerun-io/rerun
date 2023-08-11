// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/mat3x3.fbs"

#pragma once

#include "vec3d.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>

namespace rerun {
    namespace datatypes {
        /// A 3x3 column-major Matrix.
        struct Mat3x3 {
            float coeffs[9];

          public:
            // Extensions to generated type defined in 'mat3x3_ext.cpp'

            static const Mat3x3 IDENTITY;

            /// Creates a new 3x3 matrix from 3 *columns* of 3 elements each.
            Mat3x3(const Vec3D (&columns)[3])
                : coeffs{
                      columns[0].x(),
                      columns[0].y(),
                      columns[0].z(),
                      columns[1].x(),
                      columns[1].y(),
                      columns[1].z(),
                      columns[2].x(),
                      columns[2].y(),
                      columns[2].z(),
                  } {}

          public:
            Mat3x3() = default;

            Mat3x3(const float (&_coeffs)[9])
                : coeffs{
                      _coeffs[0],
                      _coeffs[1],
                      _coeffs[2],
                      _coeffs[3],
                      _coeffs[4],
                      _coeffs[5],
                      _coeffs[6],
                      _coeffs[7],
                      _coeffs[8]} {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::FixedSizeListBuilder>>
                new_arrow_array_builder(arrow::MemoryPool* memory_pool);

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Mat3x3* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
