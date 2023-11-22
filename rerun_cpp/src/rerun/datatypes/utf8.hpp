// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/utf8.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace arrow {
    class DataType;
    class StringBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A string of text, encoded as UTF-8.
    struct Utf8 {
        std::string value;

      public:
        // Extensions to generated type defined in 'utf8_ext.cpp'

        /// Construct a `Utf8` from null-terminated UTF-8.
        Utf8(const char* str) : value(str) {}

        const char* c_str() const {
            return value.c_str();
        }

      public:
        Utf8() = default;

        Utf8(std::string value_) : value(std::move(value_)) {}

        Utf8& operator=(std::string value_) {
            value = std::move(value_);
            return *this;
        }
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::Utf8> {
        static constexpr const char Name[] = "rerun.datatypes.Utf8";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StringBuilder* builder, const datatypes::Utf8* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::datatypes::Utf8` components.
        static Result<rerun::DataCell> to_arrow(
            const datatypes::Utf8* instances, size_t num_instances
        );
    };
} // namespace rerun
