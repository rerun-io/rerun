// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/component_column_selector.fbs".

#pragma once

#include "../../blueprint/datatypes/component_column_selector.hpp"
#include "../../component_descriptor.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: Describe a component column to be selected in the dataframe view.
    struct ComponentColumnSelector {
        rerun::blueprint::datatypes::ComponentColumnSelector selector;

      public:
        ComponentColumnSelector() = default;

        ComponentColumnSelector(rerun::blueprint::datatypes::ComponentColumnSelector selector_)
            : selector(std::move(selector_)) {}

        ComponentColumnSelector& operator=(
            rerun::blueprint::datatypes::ComponentColumnSelector selector_
        ) {
            selector = std::move(selector_);
            return *this;
        }

        /// Cast to the underlying ComponentColumnSelector datatype
        operator rerun::blueprint::datatypes::ComponentColumnSelector() const {
            return selector;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::blueprint::datatypes::ComponentColumnSelector) ==
        sizeof(blueprint::components::ComponentColumnSelector)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::ComponentColumnSelector> {
        static constexpr ComponentDescriptor Descriptor =
            "rerun.blueprint.components.ComponentColumnSelector";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::blueprint::datatypes::ComponentColumnSelector>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::ComponentColumnSelector` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::ComponentColumnSelector* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::blueprint::datatypes::ComponentColumnSelector>::to_arrow(
                    nullptr,
                    0
                );
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::blueprint::datatypes::ComponentColumnSelector>::to_arrow(
                    &instances->selector,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
