// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/tensor_data.fbs".

#pragma once

#include "../collection.hpp"
#include "../result.hpp"
#include "tensor_buffer.hpp"
#include "tensor_dimension.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A multi-dimensional `Tensor` of data.
    ///
    /// The number of dimensions and their respective lengths is specified by the `shape` field.
    /// The dimensions are ordered from outermost to innermost. For example, in the common case of
    /// a 2D RGB Image, the shape would be `[height, width, channel]`.
    ///
    /// These dimensions are combined with an index to look up values from the `buffer` field,
    /// which stores a contiguous array of typed values.
    struct TensorData {
        rerun::Collection<rerun::datatypes::TensorDimension> shape;

        rerun::datatypes::TensorBuffer buffer;

      public:
        // Extensions to generated type defined in 'tensor_data_ext.cpp'

        TensorData(
            Collection<rerun::datatypes::TensorDimension> shape_, datatypes::TensorBuffer buffer_
        )
            : shape(std::move(shape_)), buffer(std::move(buffer_)) {}

      public:
        TensorData() = default;

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const TensorData* elements, size_t num_elements
        );
    };
} // namespace rerun::datatypes
