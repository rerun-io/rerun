#pragma once

#include <algorithm>
#include <cassert>
#include <cstdint>
#include <cstring> // std::memset
#include <utility>
#include <vector>

#include "collection.hpp"
#include "collection_adapter.hpp"
#include "compiler_utils.hpp"

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
    /// * Borrowed: ⚠️ If data is borrowed it *must* outlive its source ⚠️
    /// (in particular, the pointer to the source mustn't invalidate)
    /// * Owned: Owned data is copied into an internal std::vector
    ///
    /// Collections are either filled explicitly using `Collection::borrow` &`Collection::take_ownership`
    /// or (most commonly in user code) implicitly using the `CollectionAdapter` trait
    /// (see documentation for `CollectionAdapter` for more information on how data can be adapted).
    ///
    /// ⚠️ To ensure that passed data is not destroyed, move it into the collection using `std::move`.
    ///
    /// Other than being assignable, collections are generally immutable:
    /// there is no mutable data access in order to not violate the contract with the data lender
    /// and changes in size are not possible.
    ///
    /// ## Implementation notes:
    ///
    /// Does intentionally not implement copy construction since for the owned case this may
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
        Collection() : ownership(CollectionOwnership::Borrowed) {
            storage.borrowed.data = nullptr;
            storage.borrowed.num_instances = 0;
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
        Collection(const Collection<TElement>& other) : ownership(other.ownership) {
            switch (other.ownership) {
                case CollectionOwnership::Borrowed: {
                    storage.borrowed = other.storage.borrowed;
                    break;
                }

                case CollectionOwnership::VectorOwned: {
                    new (&storage.vector_owned) std::vector<TElement>(other.storage.vector_owned);
                    break;
                }

                default:
                    assert(false && "unreachable");
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
        Collection(Collection<TElement>&& other) : Collection() {
            swap(other);
        }

        /// Move assignment.
        void operator=(Collection<TElement>&& other) {
            // Need to disable the maybe-uninitialized here. It seems like the compiler may be confused in situations where
            // we are assigning into an unused optional from a temporary. The fact that this hits the move-assignment without
            // having called the move constructor is suspicious though and hints of an actual bug.
            //
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(this->swap(other);)
        }

        /// Construct from a initializer list of elements that are compatible with TElement.
        ///
        /// Takes ownership of the passed elements.
        /// If you want to avoid an allocation, you have to manually keep the data on the stack
        /// (e.g. as `std::array`) and construct the collection from this instead.
        ///
        /// This is not done as a `CollectionAdapter` since it tends to cause deduction issues
        /// (since there's special rules for overload resolution for initializer lists)
        Collection(std::initializer_list<TElement> data)
            : ownership(CollectionOwnership::VectorOwned) {
            // Don't assign, since the vector is in an undefined state and assigning may
            // attempt to free data.
            new (&storage.vector_owned) std::vector<TElement>(data);
        }

        /// Borrows binary compatible data into the collection from a typed pointer.
        ///
        /// Borrowed data must outlive the collection!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        /// The passed type must be binary compatible with the collection type.
        ///
        /// Since `rerun::Collection` does not provide write access, data is guaranteed to be unchanged by
        /// any function or operation taking on a `Collection`.
        template <typename T>
        static Collection<TElement> borrow(const T* data, size_t num_instances = 1) {
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

        /// Borrows binary compatible data into the collection from an untyped pointer.
        ///
        /// This version of `borrow` that takes a void pointer, omitting any checks.
        ///
        /// Borrowed data must outlive the collection!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        ///
        /// Since `rerun::Collection` does not provide write access, data is guaranteed to be unchanged by
        /// any function or operation taking on a `rerun::Collection`.
        static Collection borrow(const void* data, size_t num_instances = 1) {
            return borrow(reinterpret_cast<const TElement*>(data), num_instances);
        }

        /// Borrows binary compatible data into the collection from a vector.
        ///
        /// Borrowed data must outlive the collection!
        /// The referenced vector must not be resized and musn't be temporary.
        ///
        /// Since `rerun::Collection` does not provide write access, data is guaranteed to be unchanged by
        /// any function or operation taking on a `rerun::Collection`.
        template <typename T>
        static Collection borrow(const std::vector<T>& data) {
            return borrow(data.data(), data.size());
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
            // TODO(#4256): there should be a special path here to avoid allocating a vector.
            std::vector<TElement> elements;
            elements.emplace_back(std::move(data));
            return take_ownership(std::move(elements));
        }

        /// Takes ownership of a single element, copying it into the collection.
        static Collection<TElement> take_ownership(const TElement& data) {
            // TODO(#4256): there should be a special path here to avoid allocating a vector.
            std::vector<TElement> elements = {data};
            return take_ownership(std::move(elements));
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

                        default:
                            assert(false && "unreachable");
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

                        default:
                            assert(false && "unreachable");
                    }
                    break;
                }

                default:
                    assert(false && "unreachable");
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

                default:
                    assert(false && "unreachable");
            }
        }

        /// Returns the number of instances in this collection.
        size_t size() const {
            switch (ownership) {
                case CollectionOwnership::Borrowed:
                    return storage.borrowed.num_instances;

                case CollectionOwnership::VectorOwned:
                    return storage.vector_owned.size();

                default:
                    assert(false && "unreachable");
            }
            return 0;
        }

        /// Returns true if the collection is empty.
        bool empty() const {
            switch (ownership) {
                case CollectionOwnership::Borrowed:
                    return storage.borrowed.num_instances == 0;

                case CollectionOwnership::VectorOwned:
                    return storage.vector_owned.empty();

                default:
                    assert(false && "unreachable");
            }
            return 0;
        }

        /// Returns a raw pointer to the underlying data.
        ///
        /// Do not use this if the data is not continuous in memory!
        /// TODO(#4257): So far it always is continuous, but in the future we want to support strides!
        ///
        /// The pointer is only valid as long as backing storage is alive
        /// which is either until the collection is destroyed the borrowed source is destroyed/moved.
        const TElement* data() const {
            switch (ownership) {
                case CollectionOwnership::Borrowed:
                    return storage.borrowed.data;

                case CollectionOwnership::VectorOwned:
                    return storage.vector_owned.data();

                default:
                    assert(false && "unreachable");
            }

            // We need to return something to avoid compiler warnings.
            // But if we don't mark this as unreachable, GCC will complain that we're dereferencing null down the line.
            RR_UNREACHABLE();
            // But with this in place, MSVC complains that the return statement is not reachable (GCC/clang on the other hand need it).
#ifndef _MSC_VER
            return nullptr;
#endif
        }

        /// TODO(andreas): Return proper iterator
        const TElement* begin() const {
            return data();
        }

        /// TODO(andreas): Return proper iterator
        const TElement* end() const {
            return data() + size();
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
            return ownership;
        }

        /// Copies the data into a new `std::vector`.
        std::vector<TElement> to_vector() const& {
            std::vector<TElement> result;
            result.reserve(size());
            result.insert(result.end(), begin(), end());
            return result;
        }

        /// Copies the data into a new `std::vector`.
        ///
        /// If possible, this will move the underlying data.
        std::vector<TElement> to_vector() && {
            switch (ownership) {
                case CollectionOwnership::Borrowed: {
                    std::vector<TElement> result;
                    result.reserve(size());
                    result.insert(result.end(), begin(), end());
                    return result;
                }

                case CollectionOwnership::VectorOwned: {
                    // Ensure move constructor is called, so `storage.vector_owned` is in a valid state.
                    return std::move(storage.vector_owned);
                }

                default:
                    assert(false && "unreachable");
            }
            return std::vector<TElement>();
        }

        /// Reinterpret this collection as a collection of bytes.
        Collection<uint8_t> to_uint8() const {
            switch (ownership) {
                case CollectionOwnership::Borrowed: {
                    return Collection<uint8_t>::borrow(
                        reinterpret_cast<const uint8_t*>(data()),
                        size() * sizeof(TElement)
                    );
                }

                case CollectionOwnership::VectorOwned: {
                    auto ptr = reinterpret_cast<const uint8_t*>(data());
                    auto num_bytes = size() * sizeof(TElement);
                    return Collection<uint8_t>::take_ownership(
                        std::vector<uint8_t>(ptr, ptr + num_bytes)
                    );
                }

                default:
                    assert(false && "unreachable");
            }
            return Collection<uint8_t>();
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
                std::memset(reinterpret_cast<void*>(this), 0, sizeof(CollectionStorage));
            }

            ~CollectionStorage() {}
        };

        CollectionOwnership ownership;
        CollectionStorage<TElement> storage;
    };

    // Convenience functions for creating typed collections via explicit borrow & ownership taking.
    // These are useful to avoid having to specify the type of the collection.
    // E.g. instead of `rerun::Collection<uint8_t>::borrow(data, num_instances)`,
    // you can just write `rerun::borrow(data, num_instances)`.

    /// Borrows binary data into a `Collection` from a pointer.
    ///
    /// Borrowed data must outlive the collection!
    /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
    /// resized.)
    /// The passed type must be binary compatible with the collection type.
    ///
    /// Since `rerun::Collection` does not provide write access, data is guaranteed to be unchanged by
    /// any function or operation taking on a `Collection`.
    template <typename TElement>
    inline Collection<TElement> borrow(const TElement* data, size_t num_instances = 1) {
        return Collection<TElement>::borrow(data, num_instances);
    }

    /// Borrows binary data into the collection from a vector.
    ///
    /// Borrowed data must outlive the collection!
    /// The referenced vector must not be resized and musn't be temporary.
    ///
    /// Since `rerun::Collection` does not provide write access, data is guaranteed to be unchanged by
    /// any function or operation taking on a `rerun::Collection`.
    template <typename TElement>
    inline Collection<TElement> borrow(const std::vector<TElement>& data) {
        return Collection<TElement>::borrow(data);
    }

    /// Takes ownership of a temporary `std::vector`, moving it into the collection.
    ///
    /// Takes ownership of the data and moves it into the collection.
    template <typename TElement>
    inline Collection<TElement> take_ownership(std::vector<TElement> data) {
        return Collection<TElement>::take_ownership(std::move(data));
    }

    /// Takes ownership of a single element, moving it into the collection.
    template <typename TElement>
    inline Collection<TElement> take_ownership(TElement data) {
        return Collection<TElement>::take_ownership(std::move(data));
    }
} // namespace rerun

// Could keep this separately, but its very hard to use the collection without the basic suite of adapters.
// Needs to know about `rerun::Collection` which means that it needs to be included after `rerun::Collection` is defined.
// (it tried to include `Collection.hpp` but if that was our starting point that include wouldn't do anything)
#include "collection_adapter_builtins.hpp"
