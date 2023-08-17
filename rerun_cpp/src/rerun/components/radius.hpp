// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/radius.fbs"

#pragma once

#include "../data_cell.hpp"
#include "../result.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>

namespace rerun {
    namespace components {
        /// A Radius component.
        struct Radius {
            float value;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            Radius() = default;

            Radius(float _value) : value(std::move(_value)) {}

            Radius& operator=(float _value) {
                value = std::move(_value);
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::FloatBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::FloatBuilder* builder, const Radius* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Radius components.
            static Result<rerun::DataCell> to_data_cell(
                const Radius* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
