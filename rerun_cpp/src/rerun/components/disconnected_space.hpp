// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/disconnected_space.fbs".

#pragma once

#include "../datatypes/bool.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: Spatially disconnect this entity from its parent.
    ///
    /// Specifies that the entity path at which this is logged is spatially disconnected from its parent,
    /// making it impossible to transform the entity path into its parent's space and vice versa.
    /// It *only* applies to space views that work with spatial transformations, i.e. 2D & 3D space views.
    /// This is useful for specifying that a subgraph is independent of the rest of the scene.
    struct DisconnectedSpace {
        /// Whether the entity path at which this is logged is disconnected from its parent.
        ///
        /// Set to true to disconnect the entity from its parent.
        /// Set to false to disable the effects of this component
        /// TODO(#7121): Once a space is disconnected, it can't be re-connected again.
        rerun::datatypes::Bool is_disconnected;

      public:
        DisconnectedSpace() = default;

        DisconnectedSpace(rerun::datatypes::Bool is_disconnected_)
            : is_disconnected(is_disconnected_) {}

        DisconnectedSpace& operator=(rerun::datatypes::Bool is_disconnected_) {
            is_disconnected = is_disconnected_;
            return *this;
        }

        DisconnectedSpace(bool value_) : is_disconnected(value_) {}

        DisconnectedSpace& operator=(bool value_) {
            is_disconnected = value_;
            return *this;
        }

        /// Cast to the underlying Bool datatype
        operator rerun::datatypes::Bool() const {
            return is_disconnected;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Bool) == sizeof(components::DisconnectedSpace));

    /// \private
    template <>
    struct Loggable<components::DisconnectedSpace> {
        static constexpr const char Name[] = "rerun.components.DisconnectedSpace";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Bool>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::DisconnectedSpace` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::DisconnectedSpace* instances, size_t num_instances
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
                    &instances->is_disconnected,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
