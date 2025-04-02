// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/query_expression.fbs".

#pragma once

#include "../../component_descriptor.hpp"
#include "../../datatypes/utf8.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: An individual query expression used to filter a set of `datatypes::EntityPath`s.
    ///
    /// Each expression is either an inclusion or an exclusion expression.
    /// Inclusions start with an optional `+` and exclusions must start with a `-`.
    ///
    /// Multiple expressions are combined together as part of `archetypes::ViewContents`.
    ///
    /// The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
    /// (`/world/**` matches both `/world` and `/world/car/driver`).
    /// Other uses of `*` are not (yet) supported.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct QueryExpression {
        rerun::datatypes::Utf8 filter;

      public:
        QueryExpression() = default;

        QueryExpression(rerun::datatypes::Utf8 filter_) : filter(std::move(filter_)) {}

        QueryExpression& operator=(rerun::datatypes::Utf8 filter_) {
            filter = std::move(filter_);
            return *this;
        }

        QueryExpression(std::string value_) : filter(std::move(value_)) {}

        QueryExpression& operator=(std::string value_) {
            filter = std::move(value_);
            return *this;
        }

        /// Cast to the underlying Utf8 datatype
        operator rerun::datatypes::Utf8() const {
            return filter;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Utf8) == sizeof(blueprint::components::QueryExpression));

    /// \private
    template <>
    struct Loggable<blueprint::components::QueryExpression> {
        static constexpr ComponentDescriptor Descriptor =
            "rerun.blueprint.components.QueryExpression";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Utf8>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::QueryExpression` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::QueryExpression* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Utf8>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Utf8>::to_arrow(
                    &instances->filter,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
