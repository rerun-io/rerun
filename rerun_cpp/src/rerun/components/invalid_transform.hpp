// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/invalid_transform.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../datatypes/bool.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: Flags the transform at its entity path as invalid.
    ///
    /// Specifies that the entity path at which this is logged is spatially disconnected from its parent,
    /// making it impossible to transform the entity path into its parent's space and vice versa.
    /// This can be useful for instance to express temporarily unknown transforms.
    ///
    /// Note that by default all transforms are considered valid.
    struct InvalidTransform {
        /// Whether the entity path at which this is logged as an invalid transform to its parent.
        rerun::datatypes::Bool invalid;

      public:
        InvalidTransform() = default;

        InvalidTransform(rerun::datatypes::Bool invalid_) : invalid(invalid_) {}

        InvalidTransform& operator=(rerun::datatypes::Bool invalid_) {
            invalid = invalid_;
            return *this;
        }

        InvalidTransform(bool value_) : invalid(value_) {}

        InvalidTransform& operator=(bool value_) {
            invalid = value_;
            return *this;
        }

        /// Cast to the underlying Bool datatype
        operator rerun::datatypes::Bool() const {
            return invalid;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Bool) == sizeof(components::InvalidTransform));

    /// \private
    template <>
    struct Loggable<components::InvalidTransform> {
        static constexpr ComponentDescriptor Descriptor = "rerun.components.InvalidTransform";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Bool>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::InvalidTransform` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::InvalidTransform* instances, size_t num_instances
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
                    &instances->invalid,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
