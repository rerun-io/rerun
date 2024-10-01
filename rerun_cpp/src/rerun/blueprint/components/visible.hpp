// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/visible.fbs".

#pragma once

#include "../../datatypes/bool.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::blueprint::components {
    /// **Component**: Whether the container, view, entity or instance is currently visible.
    struct Visible {
        rerun::datatypes::Bool visible;

      public:
        Visible() = default;

        Visible(rerun::datatypes::Bool visible_) : visible(visible_) {}

        Visible& operator=(rerun::datatypes::Bool visible_) {
            visible = visible_;
            return *this;
        }

        Visible(bool value_) : visible(value_) {}

        Visible& operator=(bool value_) {
            visible = value_;
            return *this;
        }

        /// Cast to the underlying Bool datatype
        operator rerun::datatypes::Bool() const {
            return visible;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Bool) == sizeof(blueprint::components::Visible));

    /// \private
    template <>
    struct Loggable<blueprint::components::Visible> {
        static constexpr const char Name[] = "rerun.blueprint.components.Visible";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Bool>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::Visible` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::Visible* instances, size_t num_instances
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
                    &instances->visible,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
