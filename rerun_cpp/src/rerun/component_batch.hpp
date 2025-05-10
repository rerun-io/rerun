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
    struct ComponentColumn;
}

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
        /// Automatically registers the descriptor the first time it is encountered.
        template <typename T>
        static Result<ComponentBatch> from_loggable(
            const rerun::Collection<T>& components, const ComponentDescriptor& descriptor
        ) {
            static_assert(
                rerun::is_loggable<T>,
                "The given type does not implement the rerun::Loggable trait."
            );

            /// TODO(#4257) should take a rerun::Collection instead of pointer and size.
            auto array = Loggable<T>::to_arrow(components.data(), components.size());
            RR_RETURN_NOT_OK(array.error);

            return from_arrow_array(std::move(array.value), descriptor);
        }

        /// Creates a new component batch from a single component instance.
        ///
        /// Automatically registers the descriptor the first time it is encountered.
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
        /// Automatically registers the descriptor the first time it is encountered.
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
        /// Automatically registers the descriptor the first time it is encountered.
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

        /// Creates a new component batch from an already existing arrow array.
        ///
        /// Automatically registers the descriptor the first time it is encountered.
        static Result<ComponentBatch> from_arrow_array(
            std::shared_ptr<arrow::Array> array, const ComponentDescriptor& descriptor
        );

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into a `ComponentColumn`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// \param lengths The number of components in each run. for `rerun::RecordingStream::send_columns`,
        /// this specifies the number of components at each time point.
        /// The sum of the lengths must be equal to the number of components in the batch.
        Result<ComponentColumn> partitioned(const Collection<uint32_t>& lengths) &&;

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into a `ComponentColumn`.
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        Result<ComponentColumn> partitioned() &&;

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into a `ComponentColumn`.
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// \param lengths The number of components in each run. for `rerun::RecordingStream::send_columns`,
        /// this specifies the number of components at each time point.
        /// The sum of the lengths must be equal to the number of components in the batch.
        Result<ComponentColumn> partitioned(const Collection<uint32_t>& lengths) const&;

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into a `ComponentColumn`.
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        Result<ComponentColumn> partitioned() const&;

        /// Size in the number of elements the underlying arrow array contains.
        size_t length() const;

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_component_batch` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_component_batch& out_component_batch) const;
    };
} // namespace rerun
