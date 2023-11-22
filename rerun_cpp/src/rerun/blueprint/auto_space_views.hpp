// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/auto_space_views.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class BooleanBuilder;
    class DataType;
} // namespace arrow

namespace rerun::blueprint {
    /// **Blueprint**: A flag indicating space views should be automatically populated.
    ///
    /// Unstable. Used for the ongoing blueprint experimentations.
    struct AutoSpaceViews {
        bool enabled;

      public:
        AutoSpaceViews() = default;

        AutoSpaceViews(bool enabled_) : enabled(enabled_) {}

        AutoSpaceViews& operator=(bool enabled_) {
            enabled = enabled_;
            return *this;
        }
    };
} // namespace rerun::blueprint

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::AutoSpaceViews> {
        static constexpr const char Name[] = "rerun.blueprint.AutoSpaceViews";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::BooleanBuilder* builder, const blueprint::AutoSpaceViews* elements,
            size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::blueprint::AutoSpaceViews` components.
        static Result<rerun::DataCell> to_data_cell(
            const blueprint::AutoSpaceViews* instances, size_t num_instances
        );
    };
} // namespace rerun
