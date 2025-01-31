#pragma once

#include <memory> // shared_ptr

#include "collection.hpp"
#include "component_batch.hpp"
#include "component_type.hpp"
#include "error.hpp"
#include "loggable.hpp"

struct rr_component_column;

namespace rerun {
    /// Arrow-encoded data of a column of components.
    ///
    /// This is essentially an array of `rerun::ComponentBatch` with all batches
    /// continuously in a single array.
    ///
    /// \see `rerun::RecordingStream::send_columns`
    struct ComponentColumn {
        /// Arrow-encoded list array of component batches.
        std::shared_ptr<arrow::Array> array;

        /// The type of the component instances in array.
        ComponentTypeHandle component_type;

      public:
        /// Creates a new component column from a collection of component instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        ///
        /// \param components Continuous collection of components which is about to be partitioned.
        /// \param lengths The number of components in each run. for `rerun::RecordingStream::send_columns`,
        /// this specifies the number of components at each time point.
        /// The sum of the lengths must be equal to the number of components in the batch.
        /// \param descriptor Descriptor of the component type for this column.
        template <typename T>
        static Result<ComponentColumn> from_loggable_with_lengths(
            const Collection<T>& components, const Collection<uint32_t>& lengths,
            const ComponentDescriptor& descriptor = rerun::Loggable<T>::Descriptor
        ) {
            auto component_batch_result = ComponentBatch::from_loggable(components, descriptor);
            if (component_batch_result.is_err()) {
                return component_batch_result.error;
            }
            return from_batch_with_lengths(component_batch_result.value, lengths);
        }

        /// Creates a new component column from a collection of component instances where each run has a length of one.
        ///
        /// When used with `rerun::RecordingStream::send_columns`, this is equivalent to `from_loggable(components, std::vector{1, 1, ...})`.
        /// I.e. there's a single component for each time point.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        ///
        /// \param components Continuous collection of components which is about to be partitioned into runs of length one.
        /// \param descriptor Descriptor of the component type for this column.
        template <typename T>
        static Result<ComponentColumn> from_loggable(
            const Collection<T>& components,
            const ComponentDescriptor& descriptor = rerun::Loggable<T>::Descriptor
        ) {
            return ComponentColumn::from_loggable_with_lengths(
                components,
                Collection<uint32_t>::take_ownership(std::vector<uint32_t>(components.size(), 1)),
                descriptor
            );
        }

        /// Creates a new component column with a given number of archetype indicators for a given archetype type.
        template <typename Archetype>
        static Result<ComponentColumn> from_indicators(uint32_t num_indicators) {
            auto component_batch_result = ComponentBatch::from_indicator<Archetype>();
            if (component_batch_result.is_err()) {
                return component_batch_result.error;
            }
            return ComponentColumn::from_batch_with_lengths(
                component_batch_result.value,
                Collection<uint32_t>::take_ownership(std::vector<uint32_t>(num_indicators, 0))
            );
        }

        /// Creates a new component batch partition from a batch and a collection of run lengths.
        ///
        /// \param batch A batch of components which is about to be partitioned.
        /// \param lengths The number of components in each run. for `rerun::RecordingStream::send_columns`,
        /// this specifies the number of components at each time point.
        /// The sum of the lengths must be equal to the number of components in the batch.
        static Result<ComponentColumn> from_batch_with_lengths(
            ComponentBatch batch, const Collection<uint32_t>& lengths
        );

        /// Creates a new component batch partition from a batch and a collection of component offsets.
        ///
        /// \param batch A batch of components which is about to be partitioned.
        /// \param offsets An offset within `batch` for each array of components.
        /// The last offset is the total number of components in the batch. Meaning that this array has to be
        /// one element longer than the number of component runs.
        /// E.g. a `ParitionedComponentBatch` with a single component would have an offset array of `[0, 1]`.
        /// A `ComponentColumn` with 5 components divided into runs of length 2 and 3
        // would have an offset array of `[0, 2, 5]`.
        static Result<ComponentColumn> from_batch_with_offsets(
            ComponentBatch batch, Collection<uint32_t> offsets
        );

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_component_column` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_component_column& out_component_batch) const;
    };
} // namespace rerun
