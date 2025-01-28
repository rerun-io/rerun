#pragma once

#include <memory> // shared_ptr
#include <optional>
#include <unordered_map>

#include "collection.hpp"
#include "component_descriptor.hpp"
#include "component_type.hpp"
#include "error.hpp"
#include "loggable.hpp"

namespace arrow {
    class Array;
    class DataType;
} // namespace arrow

struct rr_component_batch;

namespace rerun {
    /// Arrow-encoded data of a single batch of components together with a component descriptor.
    ///
    /// Component descriptors are registered when first encountered.
    struct ComponentBatch {
        /// Arrow-encoded data of the component instances.
        std::shared_ptr<arrow::Array> array;

        /// The type of the component instances in array.
        ComponentTypeHandle component_type;

      public:
        /// Creates a new empty component batch with a given descriptor.
        template <typename T>
        static Result<ComponentBatch> empty(const ComponentDescriptor& descriptor) {
            return from_loggable(Collection<T>(), descriptor);
        }

        /// Creates a new component batch from a collection of component instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        [[deprecated("Use from_loggable(components, descriptor) (with explicit descriptor) instead"
        )]]
        static Result<ComponentBatch> from_loggable(const rerun::Collection<T>& components) {
            return from_loggable(components, Loggable<T>::Descriptor);
        }

        /// Creates a new component batch from a collection of component instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(
            const rerun::Collection<T>& components, const ComponentDescriptor& descriptor
        ) {
            static_assert(
                rerun::is_loggable<T>,
                "The given type does not implement the rerun::Loggable trait."
            );

            // The template is over the `Loggable` itself, but a single `Loggable` might have any number of
            // descriptors/tags associated with it, therefore a static `ComponentTypeHandle` is not enough,
            // we need a map.
            static std::unordered_map<ComponentDescriptorHash, ComponentTypeHandle>
                comp_types_per_descr;

            ComponentTypeHandle comp_type_handle;

            auto descr_hash = descriptor.hashed();

            auto search = comp_types_per_descr.find(descr_hash);
            if (search != comp_types_per_descr.end()) {
                comp_type_handle = search->second;
            } else {
                auto comp_type = ComponentType(descriptor, Loggable<T>::arrow_datatype());

                const Result<ComponentTypeHandle> comp_type_handle_result =
                    comp_type.register_component();
                RR_RETURN_NOT_OK(comp_type_handle_result.error);

                comp_type_handle = comp_type_handle_result.value;
                comp_types_per_descr.insert({descr_hash, comp_type_handle});
            }

            /// TODO(#4257) should take a rerun::Collection instead of pointer and size.
            auto array = Loggable<T>::to_arrow(components.data(), components.size());
            RR_RETURN_NOT_OK(array.error);

            ComponentBatch component_batch;
            component_batch.array = std::move(array.value);
            component_batch.component_type = comp_type_handle;
            return component_batch;
        }

        /// Creates a new component batch from a single component instance.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        [[deprecated("Use from_loggable(components, descriptor) (with explicit descriptor) instead"
        )]]
        static Result<ComponentBatch> from_loggable(const T& component) {
            // Collection adapter will automatically borrow for single elements, but let's do this explicitly, avoiding the extra hoop.
            const auto collection = Collection<T>::borrow(&component, 1);
            return from_loggable(collection);
        }

        /// Creates a new component batch from a single component instance.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(
            const T& component, const ComponentDescriptor& descriptor
        ) {
            // Collection adapter will automatically borrow for single elements, but let's do this explicitly, avoiding the extra hoop.
            const auto collection = Collection<T>::borrow(&component, 1);
            return from_loggable(collection, descriptor);
        }

        /// Creates a new data cell from a single optional component instance.
        ///
        /// None is represented as a data cell with 0 instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        [[deprecated("Use from_loggable(components, descriptor) (with explicit descriptor) instead"
        )]]
        static Result<ComponentBatch> from_loggable(const std::optional<T>& component) {
            if (component.has_value()) {
                return from_loggable(component.value());
            } else {
                return from_loggable(Collection<T>());
            }
        }

        /// Creates a new data cell from a single optional component instance.
        ///
        /// None is represented as a data cell with 0 instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(
            const std::optional<T>& component, const ComponentDescriptor& descriptor
        ) {
            if (component.has_value()) {
                return from_loggable(component.value(), descriptor);
            } else {
                return from_loggable(Collection<T>(), descriptor);
            }
        }

        /// Creates a new data cell from an optional collection of component instances.
        ///
        /// None is represented as a data cell with 0 instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        [[deprecated("Use from_loggable(components, descriptor) (with explicit descriptor) instead"
        )]]
        static Result<ComponentBatch> from_loggable(
            const std::optional<rerun::Collection<T>>& components
        ) {
            if (components.has_value()) {
                return from_loggable(components.value());
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
            const std::optional<rerun::Collection<T>>& components,
            const ComponentDescriptor& descriptor
        ) {
            if (components.has_value()) {
                return from_loggable(components.value(), descriptor);
            } else {
                return from_loggable(Collection<T>(), descriptor);
            }
        }

        /// Creates a new component batch for an archetype indicator.
        template <typename Archetype>
        static Result<ComponentBatch> from_indicator() {
            return ComponentBatch::from_loggable(
                typename Archetype::IndicatorComponent(),
                Loggable<typename Archetype::IndicatorComponent>::Descriptor
            );
        }

        /// Size in the number of elements the underlying arrow array contains.
        size_t length() const;

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_component_batch` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_component_batch& out_component_batch) const;
    };
} // namespace rerun
