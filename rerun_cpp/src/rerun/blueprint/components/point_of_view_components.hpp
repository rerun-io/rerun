// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/point_of_view_components.fbs".

#pragma once

#include "../../blueprint/datatypes/component_names.hpp"
#include "../../collection.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: Component(s) used as point-of-view for a query.
    struct PointOfViewComponents {
        rerun::blueprint::datatypes::ComponentNames value;

      public:
        PointOfViewComponents() = default;

        PointOfViewComponents(rerun::blueprint::datatypes::ComponentNames value_)
            : value(std::move(value_)) {}

        PointOfViewComponents& operator=(rerun::blueprint::datatypes::ComponentNames value_) {
            value = std::move(value_);
            return *this;
        }

        PointOfViewComponents(rerun::Collection<std::string> value_) : value(std::move(value_)) {}

        PointOfViewComponents& operator=(rerun::Collection<std::string> value_) {
            value = std::move(value_);
            return *this;
        }

        /// Cast to the underlying ComponentNames datatype
        operator rerun::blueprint::datatypes::ComponentNames() const {
            return value;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::blueprint::datatypes::ComponentNames) ==
        sizeof(blueprint::components::PointOfViewComponents)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::PointOfViewComponents> {
        static constexpr const char Name[] = "rerun.blueprint.components.PointOfViewComponents";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::blueprint::datatypes::ComponentNames>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::PointOfViewComponents` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::PointOfViewComponents* instances, size_t num_instances
        ) {
            return Loggable<rerun::blueprint::datatypes::ComponentNames>::to_arrow(
                &instances->value,
                num_instances
            );
        }
    };
} // namespace rerun