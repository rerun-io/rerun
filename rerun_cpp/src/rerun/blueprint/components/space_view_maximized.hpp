// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/space_view_maximized.fbs".

#pragma once

#include "../../component_descriptor.hpp"
#include "../../datatypes/uuid.hpp"
#include "../../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::blueprint::components {
    /// **Component**: Whether a view is maximized.
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
        sizeof(rerun::datatypes::Uuid) == sizeof(blueprint::components::SpaceViewMaximized)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::SpaceViewMaximized> {
        static constexpr ComponentDescriptor Descriptor =
            "rerun.blueprint.components.SpaceViewMaximized";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Uuid>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::SpaceViewMaximized` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::SpaceViewMaximized* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Uuid>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Uuid>::to_arrow(
                    &instances->space_view_id,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
