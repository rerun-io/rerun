// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/included_queries.fbs".

#pragma once

#include "../../collection.hpp"
#include "../../datatypes/uuid.hpp"
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
    /// **Component**: All the queries belonging to a given `SpaceView`.
    ///
    /// Unstable. Used for the ongoing blueprint experimentations.
    struct IncludedQueries {
        rerun::Collection<rerun::datatypes::Uuid> query_ids;

      public:
        IncludedQueries() = default;

        IncludedQueries(rerun::Collection<rerun::datatypes::Uuid> query_ids_)
            : query_ids(std::move(query_ids_)) {}

        IncludedQueries& operator=(rerun::Collection<rerun::datatypes::Uuid> query_ids_) {
            query_ids = std::move(query_ids_);
            return *this;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::components::IncludedQueries> {
        static constexpr const char Name[] = "rerun.blueprint.components.IncludedQueries";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::ListBuilder* builder, const blueprint::components::IncludedQueries* elements,
            size_t num_elements
        );

        /// Serializes an array of `rerun::blueprint:: components::IncludedQueries` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::IncludedQueries* instances, size_t num_instances
        );
    };
} // namespace rerun
