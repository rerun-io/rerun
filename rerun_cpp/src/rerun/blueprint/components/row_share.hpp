// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/row_share.fbs".

#pragma once

#include "../../component_descriptor.hpp"
#include "../../datatypes/float32.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::blueprint::components {
    /// **Component**: The layout share of a row in the container.
    struct RowShare {
        /// The layout share of a row in the container.
        rerun::datatypes::Float32 share;

      public:
        RowShare() = default;

        RowShare(rerun::datatypes::Float32 share_) : share(share_) {}

        RowShare& operator=(rerun::datatypes::Float32 share_) {
            share = share_;
            return *this;
        }

        RowShare(float value_) : share(value_) {}

        RowShare& operator=(float value_) {
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
    static_assert(sizeof(rerun::datatypes::Float32) == sizeof(blueprint::components::RowShare));

    /// \private
    template <>
    struct Loggable<blueprint::components::RowShare> {
        static constexpr ComponentDescriptor Descriptor = "rerun.blueprint.components.RowShare";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Float32>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::RowShare` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::RowShare* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Float32>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Float32>::to_arrow(
                    &instances->share,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
