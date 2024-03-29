// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/text_log_level.fbs".

#pragma once

#include "../datatypes/utf8.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::components {
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

      public:
        // Extensions to generated type defined in 'text_log_level_ext.cpp'

        /// Designates catastrophic failures.
        static const TextLogLevel Critical;

        /// Designates very serious errors.
        static const TextLogLevel Error;

        /// Designates hazardous situations.
        static const TextLogLevel Warning;

        /// Designates useful information.
        static const TextLogLevel Info;

        /// Designates lower priority information.
        static const TextLogLevel Debug;

        /// Designates very low priority, often extremely verbose, information.
        static const TextLogLevel Trace;

        /// Construct `TextLogLevel` from a null-terminated UTF8 string.
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
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Utf8) == sizeof(components::TextLogLevel));

    /// \private
    template <>
    struct Loggable<components::TextLogLevel> {
        static constexpr const char Name[] = "rerun.components.TextLogLevel";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Utf8>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::TextLogLevel` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::TextLogLevel* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Utf8>::to_arrow(&instances->value, num_instances);
        }
    };
} // namespace rerun
