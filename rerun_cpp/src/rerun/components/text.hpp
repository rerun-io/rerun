// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/components/text.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/utf8.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StringBuilder;
} // namespace arrow

namespace rerun {
    namespace components {
        /// A string of text, e.g. for labels and text documents
        struct Text {
            rerun::datatypes::Utf8 value;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            // Extensions to generated type defined in 'text_ext.cpp'

            /// Construct `Text` from a zero-terminated UTF8 string.
            Text(const char* str) : value(str) {}

            const char* c_str() const {
                return value.c_str();
            }

          public:
            Text() = default;

            Text(rerun::datatypes::Utf8 _value) : value(std::move(_value)) {}

            Text& operator=(rerun::datatypes::Utf8 _value) {
                value = std::move(_value);
                return *this;
            }

            Text(std::string arg) : value(std::move(arg)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StringBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::StringBuilder* builder, const Text* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Text components.
            static Result<rerun::DataCell> to_data_cell(
                const Text* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
