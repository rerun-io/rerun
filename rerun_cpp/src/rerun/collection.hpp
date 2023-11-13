#pragma once

#include <algorithm>
#include <cstring>

#include <utility>
#include <vector>

#include "collection.hpp"
#include "collection_adapter.hpp"
#include "warning_macros.hpp"

// TODO: remove, needed for serialization.
#include "result.hpp"
#include "serialized_component_batch.hpp"

namespace rerun {
    /// Type of ownership of a collection's data.
    ///
    /// User access to this is typically only needed for debugging and testing.
    enum class CollectionOwnership {
        /// The collection does not own the data and only has a pointer and a size.
        Borrowed,

        /// The collection batch owns the data via an std::vector.
        VectorOwned,
    };

    /// Generic collection of elements that are roughly contiguous in memory.
    ///
    /// The most notable feature of the `rerun::Collection` is that its data may be either **owned** or **borrowed**:
    /// * Borrowed: If data is borrowed it *must* outlive its source (in particular, the pointer to
    /// the source musn't invalidate)
    /// * Owned: Owned data is copied into an internal std::vector
    ///         TODO: don't do std::vector
    ///
    /// Collection are either filled explicitly using `Collection::borrow` &`Collection::take_ownership`
    /// or (most commonly in user code) implicitly using the `CollectionAdapter` trait
    /// (see documentation for `CollectionAdapter` for more information on how data can be adapted).
    ///
    /// ## Implementation notes:
    ///
    /// Does intentionally not implement copy construction since this for the owned case this may
    /// be expensive. Typically, there should be no need to copy rerun collections, so this more
    /// than likely indicates a bug inside the Rerun SDK.
    template <typename TElement>
    class Collection {
      public:
        using ElementType = TElement;

        /// Type of an adapter given input types Ts.
        template <typename T>
        using Adapter = CollectionAdapter<
            TElement, std::remove_cv_t<std::remove_reference_t<T>>, std::enable_if_t<true>>;

        /// Creates a new empty collection.
        Collection() : ownership(CollectionOwnership::Borrowed) {
            storage.borrowed.data = nullptr;
            storage.borrowed.num_instances = 0;
        }

        /// Construct using a `CollectionAdapter` for the given input type.
        template <typename T>
        Collection(T&& input) : Collection(Adapter<T>()(std::forward<T>(input))) {}

        /// Construct from a temporary list of elements.
        ///
        /// Takes ownership of the passed elements.
        /// If you want to avoid an allocation, you have to manually keep the data on the stack
        /// (e.g. as `std::array`) and construct the collection from this instead.
        ///
        /// This is not done as a `CollectionAdapter` since it tends to cause deduction issues.
        Collection(std::initializer_list<TElement> data)
            : ownership(CollectionOwnership::VectorOwned) {
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&storage.vector_owned) std::vector<TElement>(data);
        }

        /// Borrows binary compatible data into the collection.
        ///
        /// Borrowed data must outlive the collection!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        /// The passed type must be binary compatible with the collection type.
        template <typename T>
        static Collection<TElement> borrow(const T* data, size_t num_instances) {
            static_assert(
                sizeof(T) == sizeof(TElement),
                "T & TElement are not binary compatible: Size mismatch."
            );
            static_assert(
                alignof(T) <= alignof(TElement),
                "T & TElement are not binary compatible: TElement has a higher alignment requirement than T. This implies that pointers to T may not have the alignment needed to access TElement."
            );

            Collection<TElement> batch;
            batch.ownership = CollectionOwnership::Borrowed;
            batch.storage.borrowed.data = reinterpret_cast<const TElement*>(data);
            batch.storage.borrowed.num_instances = num_instances;
            return batch;
        }

        /// Borrows binary compatible data into the collection.
        ///
        /// Version of `borrow` that takes a void pointer, omitting any checks.
        ///
        /// Borrowed data must outlive the collection!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        static Collection borrow(const void* data, size_t num_instances) {
            return borrow(reinterpret_cast<const TElement*>(data), num_instances);
        }

        /// Takes ownership of a temporary `std::vector`, moving it into the collection.
        ///
        /// Takes ownership of the data and moves it into the collection.
        static Collection<TElement> take_ownership(std::vector<TElement>&& data) {
            Collection<TElement> batch;
            batch.ownership = CollectionOwnership::VectorOwned;
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&batch.storage.vector_owned) std::vector<TElement>(std::move(data));

            return batch;
        }

        /// Takes ownership of a single element, moving it into the collection.
        static Collection<TElement> take_ownership(TElement&& data) {
            // TODO(andreas): there should be a special path here to avoid allocating a vector.
            return take_ownership(std::vector<TElement>{std::move(data)});
        }

        /// Move constructor.
        Collection(Collection<TElement>&& other) : Collection() {
            swap(other);
        }

        /// Move assignment.
        void operator=(Collection<TElement>&& other) {
            // Need to disable the maybe-uninitialized here.  It seems like the compiler may be confused in situations where
            // we are assigning into an unused optional from a temporary. The fact that this hits the move-assignment without
            // having called the move constructor is suspicious though and hints of an actual bug.
            //
            // See: https://github.com/rerun-io/rerun/issues/4027
            WITH_MAYBE_UNINITIALIZED_DISABLED(this->swap(other);)
        }

        /// Swaps the content of this collection with another.
        void swap(Collection<TElement>& other) {
            // (writing out this-> here to make it less confusing!)
            switch (this->ownership) {
                case CollectionOwnership::Borrowed: {
                    switch (other.ownership) {
                        case CollectionOwnership::Borrowed:
                            std::swap(this->storage.borrowed, other.storage.borrowed);
                            break;

                        case CollectionOwnership::VectorOwned: {
                            auto this_borrowed_data_old = this->storage.borrowed;
                            new (&this->storage.vector_owned)
                                std::vector<TElement>(std::move(other.storage.vector_owned));
                            other.storage.borrowed = this_borrowed_data_old;
                            break;
                        }
                    }
                    break;
                }

                case CollectionOwnership::VectorOwned: {
                    switch (other.ownership) {
                        case CollectionOwnership::Borrowed: {
                            auto other_borrowed_data_old = other.storage.borrowed;
                            new (&other.storage.vector_owned)
                                std::vector<TElement>(std::move(this->storage.vector_owned));
                            this->storage.borrowed = other_borrowed_data_old;
                            break;
                        }

                        case CollectionOwnership::VectorOwned:
                            std::swap(storage.vector_owned, other.storage.vector_owned);
                            break;
                    }
                    break;
                }
            }

            std::swap(ownership, other.ownership);
        }

        ~Collection() {
            switch (ownership) {
                case CollectionOwnership::Borrowed:
                    break; // nothing to do.
                case CollectionOwnership::VectorOwned:
                    storage.vector_owned.~vector(); // Deallocate the vector!
                    break;
            }
        }

        /// Copy constructor.
        Collection(const Collection<TElement>&) = delete;

        /// Returns the number of instances in this collection.
        size_t size() const {
            switch (ownership) {
                case CollectionOwnership::Borrowed:
                    return storage.borrowed.num_instances;
                case CollectionOwnership::VectorOwned:
                    return storage.vector_owned.size();
            }
            return 0;
        }

        /// Serializes the component batch into a rerun datacell that can be sent to a store.
        Result<SerializedComponentBatch> serialize() const {
            // TODO(#3794): Invert this relationship - a user of this *container* should call
            // TElement::serialize (or similar) passing in this container.
            switch (this->ownership) {
                case CollectionOwnership::Borrowed: {
                    auto cell_result = TElement::to_data_cell(
                        this->storage.borrowed.data,
                        this->storage.borrowed.num_instances
                    );
                    RR_RETURN_NOT_OK(cell_result.error);
                    return SerializedComponentBatch(
                        this->storage.borrowed.num_instances,
                        std::move(cell_result.value)
                    );
                }

                case CollectionOwnership::VectorOwned: {
                    auto cell_result = TElement::to_data_cell(
                        this->storage.vector_owned.data(),
                        this->storage.vector_owned.size()
                    );
                    RR_RETURN_NOT_OK(cell_result.error);
                    return SerializedComponentBatch(
                        this->storage.vector_owned.size(),
                        std::move(cell_result.value)
                    );
                }
            }

            return Error(ErrorCode::Unknown, "Invalid ownership state");
        }

        /// Returns the data ownership of collection.
        ///
        /// This is usually only needed for debugging and testing.
        CollectionOwnership get_ownership() const {
            return ownership;
        }

      private:
        template <typename T>
        union CollectionStorage {
            struct {
                const T* data;
                size_t num_instances;
            } borrowed;

            std::vector<T> vector_owned;

            CollectionStorage() {
                memset(reinterpret_cast<void*>(this), 0, sizeof(CollectionStorage));
            }

            ~CollectionStorage() {}
        };

        CollectionOwnership ownership;
        CollectionStorage<TElement> storage;
    };
} // namespace rerun
