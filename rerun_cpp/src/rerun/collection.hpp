#pragma once

#include <algorithm>
#include <array>
#include <cassert>
#include <cstring> // std::memset
#include <utility>
#include <vector>

#include "collection.hpp"
#include "collection_adapter.hpp"
#include "compiler_utils.hpp"

#ifndef RERUN_COLLECTION_SMALL_BUFFER_CAPACITY
/// Amount of bytes that rerun::Collection can store without allocating.
///
/// Should be at least as large as the other storage variants.
#define RERUN_COLLECTION_SMALL_BUFFER_CAPACITY (sizeof(void*) * 2)
#endif

namespace rerun {
    /// Type of ownership of a collection's data.
    ///
    /// User access to this is typically only needed for debugging and testing.
    enum class CollectionOwnership : uint8_t {
        /// The collection does not own the data and only has a pointer and a size.
        Borrowed,

        /// The collection owns the data via an std::vector.
        VectorOwned,

        /// The collection owns the data via a small buffer that's part if its payload.
        SmallBufOwned,
    };

    /// Generic collection of elements that are roughly contiguous in memory.
    ///
    /// The most notable feature of the `rerun::Collection` is that its data may be either **owned** or **borrowed**:
    /// * Borrowed: If data is borrowed it *must* outlive its source (in particular, the pointer to
    /// the source musn't invalidate)
    /// * Owned: Owned data is copied into an internal std::vector
    ///         TODO(#3794): don't do std::vector
    ///
    /// Collection are either filled explicitly using `Collection::borrow` &`Collection::take_ownership`
    /// or (most commonly in user code) implicitly using the `CollectionAdapter` trait
    /// (see documentation for `CollectionAdapter` for more information on how data can be adapted).
    ///
    /// Other than being assignable, collections are generally immutable:
    /// there is no mutable data access in order to not violate the contract with the data lender
    /// and changes in size are not possible.
    ///
    /// ## Implementation notes:
    ///
    /// Does intentionally not implement copy construction since this for the owned case this may
    /// be expensive. Typically, there should be no need to copy rerun collections, so this more
    /// than likely indicates a bug inside the Rerun SDK.
    template <typename TElement>
    class Collection {
      public:
        /// Type of the elements in the collection.
        ///
        /// Note that calling this `value_type` makes it compatible with the STL.
        using value_type = TElement;

        /// Type of an adapter given an input container type.
        ///
        /// Note that the "container" passed may also be a single element of something.
        /// The only thing relevant is that there's an Adapter for it.
        template <typename TContainer>
        using Adapter = CollectionAdapter<
            TElement, std::remove_cv_t<std::remove_reference_t<TContainer>>,
            std::enable_if_t<true>>;

        /// Creates a new empty collection.
        Collection() : _num_instances(0), _ownership(CollectionOwnership::Borrowed) {
            _storage.borrowed = nullptr;
        }

        /// Construct using a `CollectionAdapter` for the given input type.
        template <
            typename TContainer, //
            // Avoid conflicting with the copy/move constructor.
            // We could implement this also with an adapter, but this might confuse trait checks like `std::is_copy_constructible`.
            typename = std::enable_if_t<
                !std::is_same_v<std::remove_reference_t<TContainer>, Collection<TElement>>> //
            >
        Collection(TContainer&& input)
            : Collection(Adapter<TContainer>()(std::forward<TContainer>(input))) {}

        /// Copy constructor.
        ///
        /// If the data is owned, this will copy the data.
        /// If the data is borrowed, this will copy the borrow,
        /// meaning there's now (at least) two collections borrowing the same data.
        Collection(const Collection<TElement>& other)
            : _num_instances(other._num_instances), _ownership(other._ownership) {
            switch (other._ownership) {
                case CollectionOwnership::Borrowed: {
                    _storage.borrowed = other._storage.borrowed;
                    break;
                }

                case CollectionOwnership::VectorOwned: {
                    _storage.vector = other._storage.vector;
                    break;
                }

                case CollectionOwnership::SmallBufOwned: {
                    for (size_t i = 0; i < _num_instances; ++i) {
                        new (&_storage.small_buf[i]) TElement(other._storage.small_buf[i]);
                    }
                } break;
            }
        }

        /// Copy assignment.
        ///
        /// If the data is owned, this will copy the data.
        /// If the data is borrowed, this will copy the borrow,
        /// meaning there's now (at least) two collections borrowing the same data.
        void operator=(const Collection<TElement>& other) {
            this->~Collection<TElement>();
            new (this) Collection(other);
        }

        /// Move constructor.
        Collection(Collection<TElement>&& other)
            : _num_instances(other._num_instances), _ownership(other._ownership) {
            switch (_ownership) {
                case CollectionOwnership::Borrowed:
                    _storage.borrowed = other._storage.borrowed;
                    break;

                case CollectionOwnership::VectorOwned: {
                    new (&_storage.vector) std::vector<TElement>(std::move(other._storage.vector));
                    break;
                }

                case CollectionOwnership::SmallBufOwned: {
                    for (size_t i = 0; i < _num_instances; ++i) {
                        new (&_storage.small_buf[i])
                            TElement(std::move(other._storage.small_buf[i]));
                    }
                } break;
            }

            // set the other to borrowed, so it doesn't deallocate anything
            other._ownership = CollectionOwnership::Borrowed;
            other._num_instances = 0;
        }

        /// Move assignment.
        void operator=(Collection<TElement>&& other) {
            // Need to disable the maybe-uninitialized here.  It seems like the compiler may be confused in situations where
            // we are assigning into an unused optional from a temporary. The fact that this hits the move-assignment without
            // having called the move constructor is suspicious though and hints of an actual bug.
            //
            // See: https://github.com/rerun-io/rerun/issues/4027
            RERUN_WITH_MAYBE_UNINITIALIZED_DISABLED(this->swap(other);)
        }

        /// Construct from a initializer list£ of elements that are compatible with TElement.
        ///
        /// Takes ownership of the passed elements.
        /// If you want to avoid an allocation, you have to manually keep the data on the stack
        /// (e.g. as `std::array`) and construct the collection from this instead.
        ///
        /// This is not done as a `CollectionAdapter` since it tends to cause deduction issues
        /// (since there's special rules for overload resolution for initializer lists)
        Collection(std::initializer_list<TElement> data)
            : _num_instances(data.size()), _ownership(CollectionOwnership::VectorOwned) {
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            // TODO: use smallbuf optimization if applicable.
            new (&_storage.vector) std::vector<TElement>(data);
        }

        /// Borrows binary compatible data into the collection.
        ///
        /// Borrowed data must outlive the collection!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        /// The passed type must be binary compatible with the collection type.
        ///
        /// Since `rerun::Collection` does not provide write access, data is guaranteed to be unchanged by
        /// any function or operation taking on a `Collection`.
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

            Collection<TElement> collection;
            collection._storage.borrowed = reinterpret_cast<const TElement*>(data);
            collection._ownership = CollectionOwnership::Borrowed;
            collection._num_instances = num_instances;
            return collection;
        }

        /// Borrows binary compatible data into the collection.
        ///
        /// Version of `borrow` that takes a void pointer, omitting any checks.
        ///
        /// Borrowed data must outlive the collection!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        ///
        /// Since `rerun::Collection` does not provide write access, data is guaranteed to be unchanged by
        /// any function or operation taking on a `rerun::Collection`.
        static Collection borrow(const void* data, size_t num_instances) {
            return borrow(reinterpret_cast<const TElement*>(data), num_instances);
        }

        /// Takes ownership of a temporary `std::vector`, moving it into the collection.
        ///
        /// Takes ownership of the data and moves it into the collection.
        static Collection<TElement> take_ownership(std::vector<TElement>&& data) {
            Collection<TElement> batch;
            batch._num_instances = data.size();
            batch._ownership = CollectionOwnership::VectorOwned;
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&batch._storage.vector) std::vector<TElement>(std::move(data));

            return batch;
        }

        /// Takes ownership of a single element, moving it into the collection.
        static Collection<TElement> take_ownership(TElement&& data) {
            // TODO(andreas): there should be a special path here to avoid allocating a vector.
            std::vector<TElement> elements;
            elements.emplace_back(std::move(data));
            return take_ownership(std::move(elements));
        }

        // TODO: what about having several elements that fit into the small buffer.

        /// Swaps the content of this collection with another.
        void swap(Collection<TElement>& other) {
            auto num_instances_old = _num_instances;
            auto ownership_old = _ownership;

            // By using the collection storage we sidestep the need to default init TElement.
            CollectionStorage<TElement> storage_old;
            switch (_ownership) {
                case CollectionOwnership::Borrowed: {
                    storage_old.borrowed = _storage.borrowed;
                    new (this) Collection<TElement>(std::move(other));
                    other._storage.borrowed = storage_old.borrowed;
                    break;
                }

                case CollectionOwnership::VectorOwned: {
                    storage_old.vector = std::move(_storage.vector);
                    new (this) Collection<TElement>(std::move(other));
                    new (&other._storage.vector)
                        std::vector<TElement>(std::move(storage_old.vector));
                    break;
                }

                case CollectionOwnership::SmallBufOwned: {
                    for (size_t i = 0; i < _num_instances; ++i) {
                        new (&storage_old.small_buf[i])
                            TElement(std::move(other._storage.small_buf[i]));
                    }

                    new (this) Collection<TElement>(std::move(other));

                    for (size_t i = 0; i < _num_instances; ++i) {
                        new (&other._storage.small_buf[i])
                            TElement(std::move(storage_old.small_buf[i]));
                    }
                    break;
                }
            }

            other._num_instances = num_instances_old;
            other._ownership = ownership_old;
        }

        ~Collection() {
            switch (_ownership) {
                case CollectionOwnership::Borrowed:
                    break; // nothing to do.
                case CollectionOwnership::VectorOwned:
                    _storage.vector.~vector(); // Deallocate the vector!
                    break;
                case CollectionOwnership::SmallBufOwned:
                    for (size_t i = 0; i < _num_instances; ++i) {
                        _storage.small_buf[i].~TElement();
                    }
                    break;
            }
        }

        /// Returns the number of instances in this collection.
        size_t size() const {
            return _num_instances;
        }

        /// Returns a raw pointer to the underlying data.
        ///
        /// Do not use this if the data is not continuous in memory!
        /// TODO(#4225): So far it always is continuous, but in the future we want to support strides!
        ///
        /// The pointer is only valid as long as backing storage is alive
        /// which is either until the collection is destroyed the borrowed source is destroyed/moved.
        const TElement* data() const {
            switch (_ownership) {
                case CollectionOwnership::Borrowed:
                    return _storage.borrowed;
                case CollectionOwnership::VectorOwned:
                    return _storage.vector.data();
                case CollectionOwnership::SmallBufOwned:
                    return _storage.small_buf.data();
            }

            // We need to return something to avoid compiler warnings.
            // But if we don't mark this as unreachable, GCC will complain that we're dereferencing null down the line.
            RERUN_UNREACHABLE();
            return nullptr;
        }

        /// TODO(andreas): Return proper iterator
        const TElement* begin() const {
            return data();
        }

        /// TODO(andreas): Return proper iterator
        const TElement* end() const {
            return data() + _num_instances;
        }

        /// Random read access to the underlying data.
        const TElement& operator[](size_t i) const {
            assert(i < size());
            return data()[i];
        }

        /// Returns the data ownership of collection.
        ///
        /// This is usually only needed for debugging and testing.
        CollectionOwnership get_ownership() const {
            return _ownership;
        }

        /// Copies the data into a new `std::vector`.
        std::vector<TElement> to_vector() const {
            // TODO(andreas): Overload this for `const &` and `&&` to avoid the copy when possible.
            std::vector<TElement> result;
            result.reserve(size());
            result.insert(result.end(), begin(), end());
            return result;
        }

      private:
        template <typename T>
        union CollectionStorage {
            const T* borrowed;

            // TODO(andreas): don't be vector!
            std::vector<T> vector;

            /// How many elements can be stored in the small optimization buffer.
            /// Naturally, this can be zero! Luckily std::array has a special case for this.
            static constexpr size_t small_buf_capacity =
                RERUN_COLLECTION_SMALL_BUFFER_CAPACITY / sizeof(T);

            std::array<T, small_buf_capacity> small_buf;

            CollectionStorage() {
                std::memset(reinterpret_cast<void*>(this), 0, sizeof(CollectionStorage));
            }

            ~CollectionStorage() {}
        };

        CollectionStorage<TElement> _storage;

        // TODO(andreas): Fuse num instances and ownership in memory.
        size_t _num_instances;
        CollectionOwnership _ownership;
    };
} // namespace rerun

// Could keep this separately, but its very hard to use the collection without the basic suite of adapters.
// Needs to know about `rerun::Collection` which means that it needs to be included after `rerun::Collection` is defined.
// (it tried to include `Collection.hpp` but if that was our starting point that include wouldn't do anything)
#include "collection_adapter_builtins.hpp"
