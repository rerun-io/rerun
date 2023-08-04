// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/keypoint_id.fbs"

#pragma once

#include "../data_cell.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>

namespace rerun {
    namespace components {
        /// A 16-bit ID representing a type of semantic keypoint within a class.
        struct KeypointId {
            uint16_t id;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            KeypointId() = default;

            KeypointId(uint16_t _id) : id(std::move(_id)) {}

            KeypointId& operator=(uint16_t _id) {
                id = std::move(_id);
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::UInt16Builder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::UInt16Builder* builder, const KeypointId* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of KeypointId components.
            static arrow::Result<rerun::DataCell> to_data_cell(
                const KeypointId* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
