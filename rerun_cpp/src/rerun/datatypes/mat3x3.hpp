// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/mat3x3.fbs".

#pragma once

#include "../result.hpp"
#include "vec3d.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// A 3x3 Matrix.
        ///
        /// Matrices in Rerun are stored as flat list of coefficients in column-major order:
        /// ```text
        ///             column 0       column 1       column 2
        ///        -------------------------------------------------
        /// row 0 | flat_columns[0] flat_columns[3] flat_columns[6]
        /// row 1 | flat_columns[1] flat_columns[4] flat_columns[7]
        /// row 2 | flat_columns[2] flat_columns[5] flat_columns[8]
        /// ```
        struct Mat3x3 {
            /// Flat list of matrix coefficients in column-major order.
            float flat_columns[9];

          public:
            // Extensions to generated type defined in 'mat3x3_ext.cpp'

            static const Mat3x3 IDENTITY;

            /// Creates a new 3x3 matrix from 3 *columns* of 3 elements each.
            Mat3x3(const Vec3D (&columns)[3])
                : flat_columns{
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

            Mat3x3(const float (&_flat_columns)[9])
                : flat_columns{
                      _flat_columns[0],
                      _flat_columns[1],
                      _flat_columns[2],
                      _flat_columns[3],
                      _flat_columns[4],
                      _flat_columns[5],
                      _flat_columns[6],
                      _flat_columns[7],
                      _flat_columns[8]} {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Mat3x3* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
