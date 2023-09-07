// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/components/tensor_data.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/tensor_data.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace components {
        struct TensorData {
            rerun::datatypes::TensorData data;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            TensorData() = default;

            TensorData(rerun::datatypes::TensorData _data) : data(std::move(_data)) {}

            TensorData& operator=(rerun::datatypes::TensorData _data) {
                data = std::move(_data);
                return *this;
            }

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

            /// Creates a Rerun DataCell from an array of TensorData components.
            static Result<rerun::DataCell> to_data_cell(
                const TensorData* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
