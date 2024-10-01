// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/lock_range_during_zoom.fbs".

#pragma once

#include "../../datatypes/bool.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::blueprint::components {
    /// **Component**: Indicate whether the range should be locked when zooming in on the data.
    ///
    /// Default is `false`, i.e. zoom will change the visualized range.
    struct LockRangeDuringZoom {
        rerun::datatypes::Bool lock_range;

      public:
        LockRangeDuringZoom() = default;

        LockRangeDuringZoom(rerun::datatypes::Bool lock_range_) : lock_range(lock_range_) {}

        LockRangeDuringZoom& operator=(rerun::datatypes::Bool lock_range_) {
            lock_range = lock_range_;
            return *this;
        }

        LockRangeDuringZoom(bool value_) : lock_range(value_) {}

        LockRangeDuringZoom& operator=(bool value_) {
            lock_range = value_;
            return *this;
        }

        /// Cast to the underlying Bool datatype
        operator rerun::datatypes::Bool() const {
            return lock_range;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::Bool) == sizeof(blueprint::components::LockRangeDuringZoom)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::LockRangeDuringZoom> {
        static constexpr const char Name[] = "rerun.blueprint.components.LockRangeDuringZoom";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Bool>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::LockRangeDuringZoom` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::LockRangeDuringZoom* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Bool>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Bool>::to_arrow(
                    &instances->lock_range,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
