#pragma once

#include <memory> // shared_ptr

#include "collection.hpp"
#include "component_batch.hpp"
#include "component_type.hpp"
#include "error.hpp"
#include "loggable.hpp"

struct rr_partitioned_component_batch;

namespace rerun {
    /// Arrow-encoded data of a component batch partitioned into several runs of components.
    ///
    /// This is essentially an array of `rerun::ComponentBatch` with all batches
    /// continuously in a single array.
    ///
    /// \see `rerun::RecordingStream::send_columns`
    struct PartitionedComponentBatch {
        /// Arrow-encoded list array of component batches.
        std::shared_ptr<arrow::Array> array;

        /// The type of the component instances in array.
        ComponentTypeHandle component_type;

      public:
        /// Creates a new partitioned component batch from a collection of component instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        ///
        /// \param components Continuous collection of components which is about to be partitioned.
        /// \param lengths The number of components in each run. for `rerun::RecordingStream::send_columns`,
        /// this specifies the number of components at each time point.
        /// The sum of the lengths must be equal to the number of components in the batch.
        template <typename T>
        static Result<PartitionedComponentBatch> from_loggable_with_lengths(
            const Collection<T>& components, const Collection<uint32_t>& lengths
        ) {
            auto component_batch_result = ComponentBatch::from_loggable(components);
            if (component_batch_result.is_err()) {
                return component_batch_result.error;
            }
            return from_batch_with_lengths(
                component_batch_result.value,
                lengths,
                list_array_type_for<T>()
            );
        }

        /// Creates a new partitioned component batch from a collection of component instances where each run has a length of one.
        ///
        /// When used with `rerun::RecordingStream::send_columns`, this is equivalent to `from_loggable(components, std::vector{1, 1, ...})`.
        /// I.e. there's a single component for each time point.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        ///
        /// \param components Continuous collection of components which is about to be partitioned into runs of length one.
        template <typename T>
        static Result<PartitionedComponentBatch> from_loggable(const Collection<T>& components) {
            return PartitionedComponentBatch::from_loggable_with_lengths(
                components,
                Collection<uint32_t>::take_ownership(std::vector<uint32_t>(components.size(), 1))
            );
        }

        /// Creates a new partitioned component batch with a given number of archetype indicators for a given archetype type.
        template <typename Archetype>
        static Result<PartitionedComponentBatch> from_indicators(uint32_t num_indicators) {
            return PartitionedComponentBatch::from_loggable<Archetype::IndicatorComponent>(
                std::vector(num_indicators, Archetype::IndicatorComponent())
            );
        }

        /// Creates a new component batch partition from a batch and a collection of run lengths.
        ///
        /// \param batch A batch of components which is about to be partitioned.
        /// \param lengths The number of components in each run. for `rerun::RecordingStream::send_columns`,
        /// this specifies the number of components at each time point.
        /// The sum of the lengths must be equal to the number of components in the batch.
        /// \param list_array_type The type of the list array to use for the partitioned component batch.
        /// Can be retrieved using `list_array_type_for<T>()`.
        static Result<PartitionedComponentBatch> from_batch_with_lengths(
            ComponentBatch batch, const Collection<uint32_t>& lengths,
            std::shared_ptr<arrow::DataType> list_array_type
        );

        /// Creates a new component batch partition from a batch and a collection of component offsets.
        ///
        /// \param batch A batch of components which is about to be partitioned.
        /// \param offsets An offset within `batch` for each array of components.
        /// The last offset is the total number of components in the batch. Meaning that this array has to be
        /// one element longer than the number of component runs.
        /// E.g. a `ParitionedComponentBatch` with a single component would have an offset array of `[0, 1]`.
        /// A `PartitionedComponentBatch` with 5 components divided into runs of length 2 and 3
        // would have an offset array of `[0, 2, 5]`.
        /// \param list_array_type The type of the list array to use for the partitioned component batch.
        /// Can be retrieved using `list_array_type_for<T>()`.
        static Result<PartitionedComponentBatch> from_batch_with_offsets(
            ComponentBatch batch, Collection<uint32_t> offsets,
            std::shared_ptr<arrow::DataType> list_array_type
        );

        /// Returns the list array type for the given loggable type.
        ///
        /// Lazily creates the type on first call and then returns a reference to it.
        template <typename T>
        static const std::shared_ptr<arrow::DataType>& list_array_type_for() {
            static_assert(
                rerun::is_loggable<T>,
                "The given type does not implement the rerun::Loggable trait."
            );
            static std::shared_ptr<arrow::DataType> data_type =
                list_array_type_for(Loggable<T>::arrow_datatype());
            return data_type;
        }

        /// Creates a new arrow::Datatype for an underlying type.
        ///
        /// To avoid repeated allocation, use the templated version of this method.
        static std::shared_ptr<arrow::DataType> list_array_type_for(
            std::shared_ptr<arrow::DataType> inner_type
        );

        /// To rerun C API component batch.
        ///
        /// The resulting `rr_partitioned_component_batch` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_partitioned_component_batch& out_component_batch) const;
    };
} // namespace rerun
