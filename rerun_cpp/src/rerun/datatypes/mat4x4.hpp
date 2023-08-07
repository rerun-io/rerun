// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/mat4x4.fbs"

#pragma once

#include <arrow/type_fwd.h>
#include <cstdint>

namespace rerun {
    namespace datatypes {
        /// A 4x4 column-major Matrix.
        struct Mat4x4 {
            float coeffs[16];

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
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::FixedSizeListBuilder>>
                new_arrow_array_builder(arrow::MemoryPool* memory_pool);

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::FixedSizeListBuilder* builder, const Mat4x4* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
