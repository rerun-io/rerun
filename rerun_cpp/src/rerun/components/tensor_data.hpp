// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/tensor_data.fbs".

#pragma once

#include "../datatypes/tensor_data.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A multi-dimensional `Tensor` with optionally named arguments.
    struct TensorData {
        rerun::datatypes::TensorData data;

      public:
        // Extensions to generated type defined in 'tensor_data_ext.cpp'

        /// New tensor data from shape and tensor buffer.
        ///
        /// \param shape Shape of the tensor.
        /// \param buffer The tensor buffer containing the tensor's data.
        TensorData(
            rerun::Collection<rerun::datatypes::TensorDimension> shape,
            rerun::datatypes::TensorBuffer buffer
        )
            : data(rerun::datatypes::TensorData(std::move(shape), std::move(buffer))) {}

        /// New tensor data from dimensions and pointer to tensor data.
        ///
        /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
        /// \param shape Shape of the tensor. Determines the number of elements expected to be in `data_`.
        /// \param data_ Target of the pointer must outlive the archetype.
        template <typename TElement>
        explicit TensorData(Collection<datatypes::TensorDimension> shape, const TElement* data_)
            : data(rerun::datatypes::TensorData(std::move(shape), data_)) {}

      public:
        TensorData() = default;

        TensorData(rerun::datatypes::TensorData data_) : data(std::move(data_)) {}

        TensorData& operator=(rerun::datatypes::TensorData data_) {
            data = std::move(data_);
            return *this;
        }

        /// Cast to the underlying TensorData datatype
        operator rerun::datatypes::TensorData() const {
            return data;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::TensorData> {
        static constexpr const char Name[] = "rerun.components.TensorData";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const components::TensorData* elements,
            size_t num_elements
        );

        /// Serializes an array of `rerun::components::TensorData` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::TensorData* instances, size_t num_instances
        );
    };
} // namespace rerun
