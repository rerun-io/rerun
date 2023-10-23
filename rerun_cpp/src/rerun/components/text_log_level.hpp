// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/text_log_level.fbs".

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
        /// **Component**: The severity level of a text log message.
        ///
        /// Recommended to be one of:
        /// * `"CRITICAL"`
        /// * `"ERROR"`
        /// * `"WARN"`
        /// * `"INFO"`
        /// * `"DEBUG"`
        /// * `"TRACE"`
        struct TextLogLevel {
            rerun::datatypes::Utf8 value;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            // Extensions to generated type defined in 'text_log_level_ext.cpp'

            /// Designates catastrophic failures.
            static const TextLogLevel CRITICAL;

            /// Designates very serious errors.
            static const TextLogLevel ERROR;

            /// Designates hazardous situations.
            static const TextLogLevel WARN;

            /// Designates useful information.
            static const TextLogLevel INFO;

            /// Designates lower priority information.
            static const TextLogLevel DEBUG;

            /// Designates very low priority, often extremely verbose, information.
            static const TextLogLevel TRACE;

            /// Construct `TextLogLevel` from a zero-terminated UTF8 string.
            TextLogLevel(const char* str) : value(str) {}

            const char* c_str() const {
                return value.c_str();
            }

          public:
            TextLogLevel() = default;

            TextLogLevel(rerun::datatypes::Utf8 value_) : value(std::move(value_)) {}

            TextLogLevel& operator=(rerun::datatypes::Utf8 value_) {
                value = std::move(value_);
                return *this;
            }

            TextLogLevel(std::string value_) : value(std::move(value_)) {}

            TextLogLevel& operator=(std::string value_) {
                value = std::move(value_);
                return *this;
            }

            /// Cast to the underlying Utf8 datatype
            operator rerun::datatypes::Utf8() const {
                return value;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StringBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::StringBuilder* builder, const TextLogLevel* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of TextLogLevel components.
            static Result<rerun::DataCell> to_data_cell(
                const TextLogLevel* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
