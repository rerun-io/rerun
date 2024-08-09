// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/time_range_query.fbs".

#pragma once

#include "../../datatypes/time_int.hpp"
#include "../../datatypes/utf8.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::blueprint::datatypes {
    /// **Datatype**: Time range query configuration for a specific timeline.
    struct TimeRangeQuery {
        /// Name of the timeline this applies to.
        rerun::datatypes::Utf8 timeline;

        /// Beginning of the time range.
        rerun::datatypes::TimeInt start;

        /// End of the time range (inclusive).
        rerun::datatypes::TimeInt end;

      public:
        TimeRangeQuery() = default;
    };
} // namespace rerun::blueprint::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::datatypes::TimeRangeQuery> {
        static constexpr const char Name[] = "rerun.blueprint.datatypes.TimeRangeQuery";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::blueprint:: datatypes::TimeRangeQuery` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::datatypes::TimeRangeQuery* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const blueprint::datatypes::TimeRangeQuery* elements,
            size_t num_elements
        );
    };
} // namespace rerun
