// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/space_view_origin.fbs".

#pragma once

#include "../../datatypes/entity_path.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: The origin of a `SpaceView`.
    struct SpaceViewOrigin {
        rerun::datatypes::EntityPath value;

      public:
        SpaceViewOrigin() = default;

        SpaceViewOrigin(rerun::datatypes::EntityPath value_) : value(std::move(value_)) {}

        SpaceViewOrigin& operator=(rerun::datatypes::EntityPath value_) {
            value = std::move(value_);
            return *this;
        }

        SpaceViewOrigin(std::string path_) : value(std::move(path_)) {}

        SpaceViewOrigin& operator=(std::string path_) {
            value = std::move(path_);
            return *this;
        }

        /// Cast to the underlying EntityPath datatype
        operator rerun::datatypes::EntityPath() const {
            return value;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::EntityPath) == sizeof(blueprint::components::SpaceViewOrigin)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::SpaceViewOrigin> {
        static constexpr const char Name[] = "rerun.blueprint.components.SpaceViewOrigin";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::EntityPath>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::SpaceViewOrigin` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::SpaceViewOrigin* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::EntityPath>::to_arrow(
                &instances->value,
                num_instances
            );
        }
    };
} // namespace rerun
