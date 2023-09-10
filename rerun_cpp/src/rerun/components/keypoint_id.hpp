// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/keypoint_id.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/keypoint_id.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    template <typename T>
    class NumericBuilder;

    class DataType;
    class MemoryPool;
    class UInt16Type;
    using UInt16Builder = NumericBuilder<UInt16Type>;
} // namespace arrow

namespace rerun {
    namespace components {
        /// A 16-bit ID representing a type of semantic keypoint within a class.
        struct KeypointId {
            rerun::datatypes::KeypointId id;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            KeypointId() = default;

            KeypointId(rerun::datatypes::KeypointId _id) : id(std::move(_id)) {}

            KeypointId& operator=(rerun::datatypes::KeypointId _id) {
                id = std::move(_id);
                return *this;
            }

            KeypointId(uint16_t arg) : id(std::move(arg)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::UInt16Builder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::UInt16Builder* builder, const KeypointId* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of KeypointId components.
            static Result<rerun::DataCell> to_data_cell(
                const KeypointId* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
