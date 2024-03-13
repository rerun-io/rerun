// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/panel_expanded.fbs".

#pragma once

#include "../../datatypes/bool.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class BooleanBuilder;
    class DataType;
} // namespace arrow

namespace rerun::blueprint::components {
    /// **Component**: Whether an application panel is expanded or not.
    struct PanelExpanded {
        rerun::datatypes::Bool expanded;

      public:
        PanelExpanded() = default;

        PanelExpanded(rerun::datatypes::Bool expanded_) : expanded(expanded_) {}

        PanelExpanded& operator=(rerun::datatypes::Bool expanded_) {
            expanded = expanded_;
            return *this;
        }

        PanelExpanded(bool value_) : expanded(value_) {}

        PanelExpanded& operator=(bool value_) {
            expanded = value_;
            return *this;
        }

        /// Cast to the underlying Bool datatype
        operator rerun::datatypes::Bool() const {
            return expanded;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::components::PanelExpanded> {
        static constexpr const char Name[] = "rerun.blueprint.components.PanelExpanded";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::BooleanBuilder* builder, const blueprint::components::PanelExpanded* elements,
            size_t num_elements
        );

        /// Serializes an array of `rerun::blueprint:: components::PanelExpanded` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::PanelExpanded* instances, size_t num_instances
        );
    };
} // namespace rerun
