// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/grid_columns.fbs".

#pragma once

#include "../../component_descriptor.hpp"
#include "../../datatypes/uint32.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::blueprint::components {
    /// **Component**: How many columns a grid container should have.
    struct GridColumns {
        /// The number of columns.
        rerun::datatypes::UInt32 columns;

      public:
        GridColumns() = default;

        GridColumns(rerun::datatypes::UInt32 columns_) : columns(columns_) {}

        GridColumns& operator=(rerun::datatypes::UInt32 columns_) {
            columns = columns_;
            return *this;
        }

        GridColumns(uint32_t value_) : columns(value_) {}

        GridColumns& operator=(uint32_t value_) {
            columns = value_;
            return *this;
        }

        /// Cast to the underlying UInt32 datatype
        operator rerun::datatypes::UInt32() const {
            return columns;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::UInt32) == sizeof(blueprint::components::GridColumns));

    /// \private
    template <>
    struct Loggable<blueprint::components::GridColumns> {
        static constexpr ComponentDescriptor Descriptor = "rerun.blueprint.components.GridColumns";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::UInt32>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::GridColumns` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::GridColumns* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::UInt32>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::UInt32>::to_arrow(
                    &instances->columns,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
