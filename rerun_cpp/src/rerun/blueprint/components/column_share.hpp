// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/column_share.fbs".

#pragma once

#include "../../datatypes/float32.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::blueprint::components {
    /// **Component**: The layout share of a column in the container.
    struct ColumnShare {
        /// The layout shares of a column in the container.
        rerun::datatypes::Float32 share;

      public:
        ColumnShare() = default;

        ColumnShare(rerun::datatypes::Float32 share_) : share(share_) {}

        ColumnShare& operator=(rerun::datatypes::Float32 share_) {
            share = share_;
            return *this;
        }

        ColumnShare(float value_) : share(value_) {}

        ColumnShare& operator=(float value_) {
            share = value_;
            return *this;
        }

        /// Cast to the underlying Float32 datatype
        operator rerun::datatypes::Float32() const {
            return share;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Float32) == sizeof(blueprint::components::ColumnShare));

    /// \private
    template <>
    struct Loggable<blueprint::components::ColumnShare> {
        static constexpr const char Name[] = "rerun.blueprint.components.ColumnShare";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Float32>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::ColumnShare` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::ColumnShare* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Float32>::to_arrow(&instances->share, num_instances);
        }
    };
} // namespace rerun
