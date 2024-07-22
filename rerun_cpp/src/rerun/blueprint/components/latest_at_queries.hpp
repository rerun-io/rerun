// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/latest_at_queries.fbs".

#pragma once

#include "../../blueprint/datatypes/latest_at_query.hpp"
#include "../../collection.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class Array;
    class DataType;
    class ListBuilder;
} // namespace arrow

namespace rerun::blueprint::components {
    /// **Component**: Component(s) used as point-of-view for a query.
    struct LatestAtQueries {
        rerun::Collection<rerun::blueprint::datatypes::LatestAtQuery> value;

      public:
        LatestAtQueries() = default;

        LatestAtQueries(rerun::Collection<rerun::blueprint::datatypes::LatestAtQuery> value_)
            : value(std::move(value_)) {}

        LatestAtQueries& operator=(
            rerun::Collection<rerun::blueprint::datatypes::LatestAtQuery> value_
        ) {
            value = std::move(value_);
            return *this;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::components::LatestAtQueries> {
        static constexpr const char Name[] = "rerun.blueprint.components.LatestAtQueries";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::blueprint:: components::LatestAtQueries` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::LatestAtQueries* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::ListBuilder* builder, const blueprint::components::LatestAtQueries* elements,
            size_t num_elements
        );
    };
} // namespace rerun