// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/clear_is_recursive.fbs".

#pragma once

#include "../datatypes/bool.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: Configures how a clear operation should behave - recursive or not.
    struct ClearIsRecursive {
        /// If true, also clears all recursive children entities.
        rerun::datatypes::Bool recursive;

      public:
        ClearIsRecursive() = default;

        ClearIsRecursive(rerun::datatypes::Bool recursive_) : recursive(recursive_) {}

        ClearIsRecursive& operator=(rerun::datatypes::Bool recursive_) {
            recursive = recursive_;
            return *this;
        }

        ClearIsRecursive(bool value_) : recursive(value_) {}

        ClearIsRecursive& operator=(bool value_) {
            recursive = value_;
            return *this;
        }

        /// Cast to the underlying Bool datatype
        operator rerun::datatypes::Bool() const {
            return recursive;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Bool) == sizeof(components::ClearIsRecursive));

    /// \private
    template <>
    struct Loggable<components::ClearIsRecursive> {
        static constexpr const char Name[] = "rerun.components.ClearIsRecursive";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Bool>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::ClearIsRecursive` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::ClearIsRecursive* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Bool>::to_arrow(&instances->recursive, num_instances);
        }
    };
} // namespace rerun
