// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/class_id.fbs".

#pragma once

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
    namespace datatypes {
        /// **Datatype**: A 16-bit ID representing a type of semantic class.
        struct ClassId {
            uint16_t id;

          public:
            ClassId() = default;

            ClassId(uint16_t id_) : id(std::move(id_)) {}

            ClassId& operator=(uint16_t id_) {
                id = std::move(id_);
                return *this;
            }

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
        };
    } // namespace datatypes
} // namespace rerun
