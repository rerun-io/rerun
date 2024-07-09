// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/datatypes/utf8_list.fbs".

#pragma once

#include "../../collection.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace arrow {
    class Array;
    class DataType;
    class ListBuilder;
} // namespace arrow

namespace rerun::blueprint::datatypes {
    /// **Datatype**: A list of strings of text, encoded as UTF-8.
    struct Utf8List {
        rerun::Collection<std::string> value;

      public:
        Utf8List() = default;

        Utf8List(rerun::Collection<std::string> value_) : value(std::move(value_)) {}

        Utf8List& operator=(rerun::Collection<std::string> value_) {
            value = std::move(value_);
            return *this;
        }
    };
} // namespace rerun::blueprint::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::datatypes::Utf8List> {
        static constexpr const char Name[] = "rerun.blueprint.datatypes.Utf8List";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::blueprint:: datatypes::Utf8List` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::datatypes::Utf8List* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::ListBuilder* builder, const blueprint::datatypes::Utf8List* elements,
            size_t num_elements
        );
    };
} // namespace rerun
