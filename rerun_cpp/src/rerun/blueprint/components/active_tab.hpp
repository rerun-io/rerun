// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/active_tab.fbs".

#pragma once

#include "../../component_descriptor.hpp"
#include "../../datatypes/entity_path.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: The active tab in a tabbed container.
    struct ActiveTab {
        /// Which tab is currently active.
        ///
        /// This should always correspond to a tab in the container.
        rerun::datatypes::EntityPath tab;

      public:
        ActiveTab() = default;

        ActiveTab(rerun::datatypes::EntityPath tab_) : tab(std::move(tab_)) {}

        ActiveTab& operator=(rerun::datatypes::EntityPath tab_) {
            tab = std::move(tab_);
            return *this;
        }

        ActiveTab(std::string path_) : tab(std::move(path_)) {}

        ActiveTab& operator=(std::string path_) {
            tab = std::move(path_);
            return *this;
        }

        /// Cast to the underlying EntityPath datatype
        operator rerun::datatypes::EntityPath() const {
            return tab;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::EntityPath) == sizeof(blueprint::components::ActiveTab));

    /// \private
    template <>
    struct Loggable<blueprint::components::ActiveTab> {
        static constexpr ComponentDescriptor Descriptor = "rerun.blueprint.components.ActiveTab";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::EntityPath>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::ActiveTab` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::ActiveTab* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::EntityPath>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::EntityPath>::to_arrow(
                    &instances->tab,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
