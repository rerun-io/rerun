// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/class_id.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/class_id.hpp"
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
        /// A 16-bit ID representing a type of semantic class.
        struct ClassId {
            rerun::datatypes::ClassId id;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            ClassId() = default;

            ClassId(rerun::datatypes::ClassId _id) : id(std::move(_id)) {}

            ClassId& operator=(rerun::datatypes::ClassId _id) {
                id = std::move(_id);
                return *this;
            }

            ClassId(uint16_t arg) : id(std::move(arg)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::UInt16Builder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::UInt16Builder* builder, const ClassId* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of ClassId components.
            static Result<rerun::DataCell> to_data_cell(
                const ClassId* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
