#pragma once

#include <memory> // shared_ptr
#include <optional>

#include "collection.hpp"
#include "component_type.hpp"
#include "error.hpp"
#include "loggable.hpp"

namespace arrow {
    class Array;
    class DataType;
} // namespace arrow

struct rr_component_batch;

namespace rerun {
    /// Arrow-encoded data of a single batch components for a single entity.
    ///
    /// Note that this doesn't own `datatype` and `component_name`.
    struct ComponentBatch {
        /// Arrow-encoded data of the component instances.
        std::shared_ptr<arrow::Array> array;

        /// The type of the component instances in array.
        ComponentTypeHandle component_type;

      public:
        /// Creates a new component batch from a collection of component instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(const rerun::Collection<T>& components) {
            static_assert(
                rerun::is_loggable<T>,
                "The given type does not implement the rerun::Loggable trait."
            );

            // Register type, only done once per type (but error check happens every time).
            static const Result<ComponentTypeHandle> component_type =
                ComponentType(Loggable<T>::Name, Loggable<T>::arrow_datatype())
                    .register_component();
            RR_RETURN_NOT_OK(component_type.error);

            /// TODO(#4257) should take a rerun::Collection instead of pointer and size.
            auto array = Loggable<T>::to_arrow(components.data(), components.size());
            RR_RETURN_NOT_OK(array.error);

            ComponentBatch component_batch;
            component_batch.array = std::move(array.value);
            component_batch.component_type = component_type.value;
            return component_batch;
        }

        /// Creates a new component batch from a single component instance.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(const T& component) {
            // Collection adapter will automatically borrow for single elements, but let's do this explicitly, avoiding the extra hoop.
            const auto collection = Collection<T>::borrow(&component, 1);
            return from_loggable(collection);
        }

        /// Creates a new data cell from a single optional component instance.
        ///
        /// None is represented as a data cell with 0 instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(const std::optional<T>& component) {
            if (component.has_value()) {
                return from_loggable(component.value());
            } else {
                return from_loggable(Collection<T>());
            }
        }

        /// Creates a new data cell from an optional collection of component instances.
        ///
        /// None is represented as a data cell with 0 instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(
            const std::optional<rerun::Collection<T>>& components
        ) {
            if (components.has_value()) {
                return from_loggable(components.value());
            } else {
                return from_loggable(Collection<T>());
            }
        }

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_component_batch` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_component_batch& out_component_batch) const;
    };
} // namespace rerun
