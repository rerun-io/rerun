#pragma once

#include <type_traits> // std::enable_if, std::false_type

namespace rerun {
    /// The `rerun::CollectionAdapter` trait is responsible for mapping an input argument to a `rerun::Collection`.
    ///
    /// There are default implementations for standard containers, as well as single
    /// elements. These can be found in `rerun/collection_adapter_builtins.hpp`.
    ///
    /// An adapter may choose to either produce an owned or borrowed collection.
    /// Borrowed collections required that a pointer to the passed in ("adapted") data
    /// outlives the collection. Owned component batches on the other hand take ownership by
    /// allocating a `std::vector` and moving the data into it. This is typically only required when
    /// passing in temporary objects into an adapter or non-trivial data conversion is necessary.
    ///
    /// By implementing your own adapters for certain component types, you can map your data to
    /// Rerun types which then can be logged.
    ///
    /// To implement an adapter for a type T, specialize `CollectionAdapter<TElement, T>` and
    /// define `Collection<TElement> operator()(const T& input)`.
    /// It is *highly recommended* to also specify `Collection<TElement> operator()(T&&
    /// input)` in order to accidentally borrow data that is passed in as a temporary!
    template <typename TElement, typename TContainer, typename Enable = std::enable_if_t<true>>
    struct CollectionAdapter {
        /// \private
        /// `NoAdapterFor` always evaluates to false, but in a way that requires template instantiation.
        template <typename... Ts>
        struct NoAdapterFor : std::false_type {};

        static_assert(
            NoAdapterFor<TElement, TContainer>::value,
            "CollectionAdapter is not implemented for this type. "
            "It is implemented for single elements as well as std::vector, std::array, and "
            "c-arrays of components. "
            "You can add your own implementation by specializing "
            "rerun::CollectionAdapter<TElement, TContainer> for a given "
            "target type TElement and your input type TContainer."
        );

        // TODO(andreas): List methods that the trait should implement.
    };
} // namespace rerun
