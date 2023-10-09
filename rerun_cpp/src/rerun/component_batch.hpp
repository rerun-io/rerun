#pragma once

#include <array>
#include <utility>
#include <vector>

#include "data_cell.hpp"
#include "result.hpp"

namespace rerun {
    /// Generic list of components that are contiguous in memory.
    ///
    /// Any creation from a non-temporary will neither copy the data nor take ownership of it.
    /// This means that any data passed in (unless temporary) must outlive the component batch!
    ///
    /// However, when created from a temporary, the component batch will take ownership of the data.
    /// For details, refer to the documentation of the respective constructor.
    ///
    /// Implementation notes:
    ///
    /// Does intentionally not implement copy construction since this for the owned case this may
    /// be expensive.Typically, there should be no need to copy component batches, so this more than
    /// likely indicates a bug inside the Rerun SDK.
    template <typename TComponent>
    class ComponentBatch {
      public:
        /// Creates a new empty component batch.
        ///
        /// Note that logging an empty component batch is different from logging no component batch:
        /// When you log an empty component batch at an entity that already has some components of
        /// the same type, it will clear out all components of that type.
        ComponentBatch() : ownership(BatchOwnership::Borrowed) {
            storage.borrowed.data = nullptr;
            storage.borrowed.num_instances = 0;
        }

        /// Construct from a single component.
        ///
        /// *Attention*: Does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component batch.
        ComponentBatch(const TComponent& one_and_only) : ComponentBatch(&one_and_only, 1) {}

        /// Construct from a single temporary component.
        ///
        /// Takes ownership of the data and moves it into the component batch.
        /// TODO(andreas): Optimize this to not allocate a vector.
        ComponentBatch(TComponent&& one_and_only) : ComponentBatch(std::vector{one_and_only}) {}

        /// Construct from a raw pointer and size.
        ///
        /// Naturally, this does not take ownership of the data.
        ComponentBatch(const TComponent* data, size_t num_instances)
            : ownership(BatchOwnership::Borrowed) {
            storage.borrowed.data = data;
            storage.borrowed.num_instances = num_instances;
        }

        /// Construct from an std::vector.
        ///
        /// *Attention*: Does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component batch.
        /// In particular, manipulating the passed vector after constructing the component batch,
        /// will invalidate it, similar to iterator invalidation.
        ComponentBatch(const std::vector<TComponent>& data)
            : ComponentBatch(data.data(), data.size()) {}

        /// Construct from a temporary std::vector.
        ///
        /// Takes ownership of the data and moves it into the component batch.
        ComponentBatch(std::vector<TComponent>&& data) : ownership(BatchOwnership::VectorOwned) {
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&storage.vector_owned) std::vector<TComponent>(data);
        }

        /// Construct from an std::array.
        ///
        /// *Attention*: Does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component batch.
        template <size_t NumInstances>
        ComponentBatch(const std::array<TComponent, NumInstances>& data)
            : ComponentBatch(data.data(), data.size()) {}

        /// Construct from a C-Array.
        ///
        /// *Attention*: Does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component batch.
        template <size_t NumInstances>
        ComponentBatch(const TComponent (&data)[NumInstances])
            : ComponentBatch(&data[0], NumInstances) {}

        /// Construct from a temporary list.
        ///
        /// Persists the list into an internal std::vector.
        /// If you want to avoid an allocation, you have to manually keep the data on the stack
        /// (e.g. as std::array) and construct the batch from this instead.
        ComponentBatch(std::initializer_list<TComponent> data)
            : ownership(BatchOwnership::VectorOwned) {
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&storage.vector_owned) std::vector<TComponent>(data);
        }

        /// Move constructor.
        ComponentBatch(ComponentBatch<TComponent>&& other) {
            ownership = other.ownership;
            switch (ownership) {
                case BatchOwnership::Borrowed:
                    storage.borrowed = other.storage.borrowed;
                    break;
                case BatchOwnership::VectorOwned:
                    // Don't assign, since the vector is in an undefined state and assigning may
                    // attempt to free data.
                    new (&storage.vector_owned)
                        std::vector<TComponent>(std::move(other.storage.vector_owned));
                    break;
            }
        }

        /// Move assignment
        void operator=(ComponentBatch<TComponent>&& other) {
            this->~ComponentBatch();
            new (this) ComponentBatch(std::move(other));
        }

        ~ComponentBatch() {
            switch (ownership) {
                case BatchOwnership::Borrowed:
                    break; // nothing to do.
                case BatchOwnership::VectorOwned:
                    storage.vector_owned.~vector(); // Deallocate the vector!
                    break;
            }
        }

        /// Returns the number of instances in this component batch.
        size_t size() const {
            switch (ownership) {
                case BatchOwnership::Borrowed:
                    return storage.borrowed.num_instances;
                case BatchOwnership::VectorOwned:
                    return storage.vector_owned.size();
            }
        }

        /// Serializes the component batch into a rerun datacell that can be sent to a store.
        Result<SerializedComponentBatch> serialize() const {
            // TODO(andreas): `to_data_cell` should actually get our storage representation passed
            // in which we'll allow "type adaptors" in the future (for e.g. setting a stride or
            // similar).
            switch (ownership) {
                case BatchOwnership::Borrowed: {
                    auto cell_result = TComponent::to_data_cell(
                        storage.borrowed.data,
                        storage.borrowed.num_instances
                    );
                    RR_RETURN_NOT_OK(cell_result.error);
                    return SerializedComponentBatch(
                        storage.borrowed.num_instances,
                        std::move(cell_result.value)
                    );
                }

                case BatchOwnership::VectorOwned: {
                    auto cell_result = TComponent::to_data_cell(
                        storage.vector_owned.data(),
                        storage.vector_owned.size()
                    );
                    RR_RETURN_NOT_OK(cell_result.error);
                    return SerializedComponentBatch(
                        storage.vector_owned.size(),
                        std::move(cell_result.value)
                    );
                }
            }
        }

      private:
        enum class BatchOwnership {
            /// The component batch does not own the data and only has a pointer and a
            /// size.
            Borrowed,

            /// The component batch owns the data via an std::vector.
            VectorOwned,
        };

        BatchOwnership ownership;

        template <typename T>
        union ComponentBatchStorage {
            struct {
                const T* data;
                size_t num_instances;
            } borrowed;

            std::vector<T> vector_owned;

            ComponentBatchStorage() {}

            ~ComponentBatchStorage() {}
        };

        ComponentBatchStorage<TComponent> storage;
    };
} // namespace rerun
