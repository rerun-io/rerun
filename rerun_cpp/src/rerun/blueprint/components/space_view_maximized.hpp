// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/space_view_maximized.fbs".

#pragma once

#include "../../datatypes/uuid.hpp"
#include "../../result.hpp"
#include "space_view_maximized.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class FixedSizeListBuilder;
}

namespace rerun::blueprint::components {
    /// **Component**: Whether a space view is maximized.
    ///
    /// Unstable. Used for the ongoing blueprint experimentations.
    struct SpaceViewMaximized {
        rerun::datatypes::Uuid space_view_id;

      public:
        SpaceViewMaximized() = default;

        SpaceViewMaximized(rerun::datatypes::Uuid space_view_id_) : space_view_id(space_view_id_) {}

        SpaceViewMaximized& operator=(rerun::datatypes::Uuid space_view_id_) {
            space_view_id = space_view_id_;
            return *this;
        }

        SpaceViewMaximized(std::array<uint8_t, 16> bytes_) : space_view_id(bytes_) {}

        SpaceViewMaximized& operator=(std::array<uint8_t, 16> bytes_) {
            space_view_id = bytes_;
            return *this;
        }

        /// Cast to the underlying Uuid datatype
        operator rerun::datatypes::Uuid() const {
            return space_view_id;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::Uuid) == sizeof(rerun::blueprint::components::SpaceViewMaximized)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::SpaceViewMaximized> {
        static constexpr const char Name[] = "rerun.blueprint.components.SpaceViewMaximized";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Uuid>::arrow_datatype();
        }

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder,
            const blueprint::components::SpaceViewMaximized* elements, size_t num_elements
        ) {
            return Loggable<rerun::datatypes::Uuid>::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::Uuid*>(elements),
                num_elements
            );
        }

        /// Serializes an array of `rerun::blueprint:: components::SpaceViewMaximized` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::SpaceViewMaximized* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Uuid>::to_arrow(
                reinterpret_cast<const rerun::datatypes::Uuid*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
