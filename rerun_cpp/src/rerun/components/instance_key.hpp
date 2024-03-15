// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/instance_key.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class Array;
    class DataType;
    class UInt64Type;
    using UInt64Builder = NumericBuilder<UInt64Type>;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A unique numeric identifier for each individual instance within a batch.
    struct InstanceKey {
        uint64_t value;

      public:
        InstanceKey() = default;

        InstanceKey(uint64_t value_) : value(value_) {}

        InstanceKey& operator=(uint64_t value_) {
            value = value_;
            return *this;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::InstanceKey> {
        static constexpr const char Name[] = "rerun.components.InstanceKey";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::InstanceKey` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::InstanceKey* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt64Builder* builder, const components::InstanceKey* elements,
            size_t num_elements
        );
    };
} // namespace rerun
