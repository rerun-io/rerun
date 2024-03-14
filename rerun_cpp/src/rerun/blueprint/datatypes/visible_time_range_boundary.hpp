// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/datatypes/visible_time_range.fbs".

#pragma once

#include "../../result.hpp"
#include "time_int.hpp"
#include "visible_time_range_boundary_kind.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::blueprint::datatypes {
    /// **Datatype**: Type of boundary for visible history.
    struct VisibleTimeRangeBoundary {
        /// Type of the boundary.
        rerun::blueprint::datatypes::VisibleTimeRangeBoundaryKind kind;

        /// Value of the boundary (ignored for `Infinite` type).
        rerun::blueprint::datatypes::TimeInt time;

      public:
        VisibleTimeRangeBoundary() = default;
    };
} // namespace rerun::blueprint::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::datatypes::VisibleTimeRangeBoundary> {
        static constexpr const char Name[] = "rerun.blueprint.datatypes.VisibleTimeRangeBoundary";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder,
            const blueprint::datatypes::VisibleTimeRangeBoundary* elements, size_t num_elements
        );

        /// Serializes an array of `rerun::blueprint:: datatypes::VisibleTimeRangeBoundary` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::datatypes::VisibleTimeRangeBoundary* instances, size_t num_instances
        );
    };
} // namespace rerun
