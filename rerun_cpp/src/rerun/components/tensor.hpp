// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/tensor.fbs"

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/tensor.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>

namespace rerun {
    namespace components {
        struct Tensor {
            rerun::datatypes::Tensor data;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            Tensor() = default;

            Tensor(rerun::datatypes::Tensor _data) : data(std::move(_data)) {}

            Tensor& operator=(rerun::datatypes::Tensor _data) {
                data = std::move(_data);
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::StructBuilder* builder, const Tensor* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Tensor components.
            static arrow::Result<rerun::DataCell> to_data_cell(
                const Tensor* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
