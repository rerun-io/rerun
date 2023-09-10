// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/mat4x4.fbs".

#pragma once

#include "../result.hpp"
#include "vec4d.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class FixedSizeListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// A 4x4 column-major Matrix.
        struct Mat4x4 {
            float coeffs[16];

          public:
            // Extensions to generated type defined in 'mat4x4_ext.cpp'

            static const Mat4x4 IDENTITY;

            /// Creates a new 4x4 matrix from 3 *columns* of 4 elements each.
            Mat4x4(const Vec4D (&_columns)[4])
                : coeffs{
                      _columns[0].x(),
                      _columns[0].y(),
                      _columns[0].z(),
                      _columns[0].w(),
                      _columns[1].x(),
                      _columns[1].y(),
                      _columns[1].z(),
                      _columns[1].w(),
                      _columns[2].x(),
                      _columns[2].y(),
                      _columns[2].z(),
                      _columns[2].w(),
                      _columns[3].x(),
                      _columns[3].y(),
                      _columns[3].z(),
                      _columns[3].w(),
                  } {}

          public:
            Mat4x4() = default;

            Mat4x4(const float (&_coeffs)[16])
                : coeffs{
                      _coeffs[0],
                      _coeffs[1],
                      _coeffs[2],
                      _coeffs[3],
                      _coeffs[4],
                      _coeffs[5],
                      _coeffs[6],
                      _coeffs[7],
                      _coeffs[8],
                      _coeffs[9],
                      _coeffs[10],
                      _coeffs[11],
                      _coeffs[12],
                      _coeffs[13],
                      _coeffs[14],
                      _coeffs[15]} {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::FixedSizeListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Mat4x4* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
