// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/tensor_data.fbs"

#pragma once

#include "../result.hpp"
#include "tensor_buffer.hpp"
#include "tensor_dimension.hpp"
#include "tensor_id.hpp"

#include <cstdint>
#include <memory>
#include <vector>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// A multi-dimensional `Tensor` of data.
        ///
        /// The number of dimensions and their respective lengths is specified by the `shape` field.
        /// The dimensions are ordered from outermost to innermost. For example, in the common case
        /// of a 2D RGB Image, the shape would be `[height, width, channel]`.
        ///
        /// These dimensions are combined with an index to look up values from the `buffer` field,
        /// which stores a contiguous array of typed values.
        struct TensorData {
            rerun::datatypes::TensorId id;

            std::vector<rerun::datatypes::TensorDimension> shape;

            rerun::datatypes::TensorBuffer buffer;

          public:
            TensorData() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const TensorData* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
