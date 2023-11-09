#pragma once

#include <algorithm>
#include <array>
#include <cstring>

#include <utility>
#include <vector>

#include "component_batch_adapter.hpp"
#include "result.hpp"
#include "serialized_component_batch.hpp"
#include "warning_macros.hpp"

namespace rerun {
    /// Type of ownership of the batch's data.
    ///
    /// User access to this is typically only needed for debugging and testing.
    enum class BatchOwnership {
        /// The component batch does not own the data and only has a pointer and a
        /// size.
        Borrowed,

        /// The component batch owns the data via an std::vector.
        VectorOwned,
    };

    /// Generic list of components that are contiguous in memory.
    ///
    /// Data in the component batch can either be borrowed or owned.
    /// * Borrowed: If data is borrowed it *must* outlive its source (in particular, the pointer to
    /// the source musn't invalidate)
    /// * Owned: Owned data is copied into an internal std::vector
    ///
    /// ComponentBatches are either filled explicitly using `ComponentBatch::borrow` or
    /// `ComponentBatch::take_ownership` or (most commonly) implicitly using the
    /// `ComponentBatchAdapter` trait (see its documentation for more information on how data can be
    /// adapted).
    ///
    /// ## Implementation notes:
    ///
    /// Does intentionally not implement copy construction since this for the owned case this may
    /// be expensive. Typically, there should be no need to copy component batches, so this more
    /// than likely indicates a bug inside the Rerun SDK.
    template <typename TComponent>
    class ComponentBatch {
      public:
        using TComponentType = TComponent;

        /// Type of an adapter given input types Ts.
        template <typename T>
        using TAdapter = ComponentBatchAdapter<
            TComponent, std::remove_cv_t<std::remove_reference_t<T>>, std::enable_if_t<true>>;

        /// Creates a new empty component batch.
        ///
        /// Note that logging an empty component batch is different from logging no component
        /// batch: When you log an empty component batch at an entity that already has some
        /// components of the same type, it will clear out all components of that type.
        ComponentBatch() : ownership(BatchOwnership::Borrowed) {
            storage.borrowed.data = nullptr;
            storage.borrowed.num_instances = 0;
        }

        /// Construct using a `ComponentBatchAdapter`.
        template <typename T>
        ComponentBatch(T&& input) : ComponentBatch(TAdapter<T>()(std::forward<T>(input))) {}

        /// Construct from a temporary list of components.
        ///
        /// Persists the list into an internal std::vector.
        /// If you want to avoid an allocation, you have to manually keep the data on the stack
        /// (e.g. as std::array) and construct the batch from this instead.
        ///
        /// This is not done as ComponentBatchAdapter since it tends to cause deduction issues.
        ComponentBatch(std::initializer_list<TComponent> data)
            : ownership(BatchOwnership::VectorOwned) {
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&storage.vector_owned) std::vector<TComponent>(data);
        }

        /// Borrows binary compatible data into the component batch.
        ///
        /// Borrowed data must outlive the component batch!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        /// The passed type must be binary compatible with the component type.
        template <typename T>
        static ComponentBatch<TComponent> borrow(const T* data, size_t num_instances) {
            static_assert(
                sizeof(T) == sizeof(TComponent),
                "T & TComponent are not binary compatible: Size mismatch."
            );
            static_assert(
                alignof(T) <= alignof(TComponent),
                "T & TComponent are not binary compatible: TComponent has a higher alignment requirement than T. This implies that pointers to T may not have the alignment needed to access TComponent."
            );

            ComponentBatch<TComponent> batch;
            batch.ownership = BatchOwnership::Borrowed;
            batch.storage.borrowed.data = reinterpret_cast<const TComponent*>(data);
            batch.storage.borrowed.num_instances = num_instances;
            return batch;
        }

        /// Borrows binary compatible data into the component batch.
        ///
        /// Version of `borrow` that takes a void pointer, omitting any checks.
        ///
        /// Borrowed data must outlive the component batch!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        static ComponentBatch borrow(const void* data, size_t num_instances) {
            return borrow(reinterpret_cast<const TComponent*>(data), num_instances);
        }

        /// Takes ownership of a temporary std::vector, moving it into the component batch.
        ///
        /// Takes ownership of the data and moves it into the component batch.
        static ComponentBatch<TComponent> take_ownership(std::vector<TComponent>&& data) {
            ComponentBatch<TComponent> batch;
            batch.ownership = BatchOwnership::VectorOwned;
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&batch.storage.vector_owned) std::vector<TComponent>(std::move(data));

            return batch;
        }

        /// Takes ownership of a single component, moving it into the component batch.
        static ComponentBatch<TComponent> take_ownership(TComponent&& data) {
            // TODO(andreas): there should be a special path here to avoid allocating a vector.
            return take_ownership(std::vector<TComponent>{std::move(data)});
        }

        /// Move constructor.
        ComponentBatch(ComponentBatch<TComponent>&& other) : ComponentBatch() {
            swap(other);
        }

        /// Move assignment
        void operator=(ComponentBatch<TComponent>&& other) {
            // Need to disable the maybe-uninitialized here.  It seems like the compiler may be confused in situations where
            // we are assigning into an unused optional from a temporary. The fact that this hits the move-assignment without
            // having called the move constructor is suspicious though and hints of an actual bug.
            //
            // See: https://github.com/rerun-io/rerun/issues/4027
            WITH_MAYBE_UNINITIALIZED_DISABLED(this->swap(other);)
        }

        /// Swaps the content of this component batch with another.
        void swap(ComponentBatch<TComponent>& other) {
            // (writing out this-> here to make it less confusing!)
            switch (this->ownership) {
                case BatchOwnership::Borrowed: {
                    switch (other.ownership) {
                        case BatchOwnership::Borrowed:
                            std::swap(this->storage.borrowed, other.storage.borrowed);
                            break;

                        case BatchOwnership::VectorOwned: {
                            auto this_borrowed_data_old = this->storage.borrowed;
                            new (&this->storage.vector_owned)
                                std::vector<TComponent>(std::move(other.storage.vector_owned));
                            other.storage.borrowed = this_borrowed_data_old;
                            break;
                        }
                    }
                    break;
                }

                case BatchOwnership::VectorOwned: {
                    switch (other.ownership) {
                        case BatchOwnership::Borrowed: {
                            auto other_borrowed_data_old = other.storage.borrowed;
                            new (&other.storage.vector_owned)
                                std::vector<TComponent>(std::move(this->storage.vector_owned));
                            this->storage.borrowed = other_borrowed_data_old;
                            break;
                        }

                        case BatchOwnership::VectorOwned:
                            std::swap(storage.vector_owned, other.storage.vector_owned);
                            break;
                    }
                    break;
                }
            }

            std::swap(ownership, other.ownership);
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

        /// Copy constructor.
        ComponentBatch(const ComponentBatch<TComponent>&) = delete;

        /// Returns the number of instances in this component batch.
        size_t size() const {
            switch (ownership) {
                case BatchOwnership::Borrowed:
                    return storage.borrowed.num_instances;
                case BatchOwnership::VectorOwned:
                    return storage.vector_owned.size();
            }
            return 0;
        }

        /// Serializes the component batch into a rerun datacell that can be sent to a store.
        Result<SerializedComponentBatch> serialize() const {
            // TODO(#3794): Invert this relationship - a user of this *container* should call
            // TComponent::serialize (or similar) passing in this container.
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

            return Error(ErrorCode::Unknown, "Invalid ownership state");
        }

        /// Returns the ownership of the component batch.
        ///
        /// This is usually only needed for debugging and testing.
        BatchOwnership get_ownership() const {
            return ownership;
        }

      private:
        template <typename T>
        union ComponentBatchStorage {
            struct {
                const T* data;
                size_t num_instances;
            } borrowed;

            std::vector<T> vector_owned;

            ComponentBatchStorage() {
                memset(reinterpret_cast<void*>(this), 0, sizeof(ComponentBatchStorage));
            }

            ~ComponentBatchStorage() {}
        };

        BatchOwnership ownership;
        ComponentBatchStorage<TComponent> storage;
    };
} // namespace rerun
