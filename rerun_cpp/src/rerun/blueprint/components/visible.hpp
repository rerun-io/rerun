// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/visible.fbs".

#pragma once

#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class BooleanBuilder;
    class DataType;
} // namespace arrow

namespace rerun::blueprint::components {
    /// **Component**: Whether the container, space view, entity or instance is currently visible.
    struct Visible {
        bool visible;

      public:
        Visible() = default;

        Visible(bool visible_) : visible(visible_) {}

        Visible& operator=(bool visible_) {
            visible = visible_;
            return *this;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::components::Visible> {
        static constexpr const char Name[] = "rerun.blueprint.components.Visible";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::blueprint:: components::Visible` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::Visible* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::BooleanBuilder* builder, const blueprint::components::Visible* elements,
            size_t num_elements
        );
    };
} // namespace rerun
