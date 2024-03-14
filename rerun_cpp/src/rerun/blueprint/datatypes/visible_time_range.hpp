// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/datatypes/visible_time_range.fbs".

#pragma once

#include "../../result.hpp"
#include "visible_time_range_boundary.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::blueprint::datatypes {
    /// **Datatype**: Visible time range bounds.
    struct VisibleTimeRange {
        /// Low time boundary for sequence timeline.
        rerun::blueprint::datatypes::VisibleTimeRangeBoundary from_sequence;

        /// High time boundary for sequence timeline.
        rerun::blueprint::datatypes::VisibleTimeRangeBoundary to_sequence;

        /// Low time boundary for time timeline.
        rerun::blueprint::datatypes::VisibleTimeRangeBoundary from_time;

        /// High time boundary for time timeline.
        rerun::blueprint::datatypes::VisibleTimeRangeBoundary to_time;

      public:
        VisibleTimeRange() = default;
    };
} // namespace rerun::blueprint::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::datatypes::VisibleTimeRange> {
        static constexpr const char Name[] = "rerun.blueprint.datatypes.VisibleTimeRange";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const blueprint::datatypes::VisibleTimeRange* elements,
            size_t num_elements
        );

        /// Serializes an array of `rerun::blueprint:: datatypes::VisibleTimeRange` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::datatypes::VisibleTimeRange* instances, size_t num_instances
        );
    };
} // namespace rerun
