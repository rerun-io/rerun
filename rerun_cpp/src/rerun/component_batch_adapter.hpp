#pragma once

namespace rerun {
    /// The `ComponentBatchAdapter` trait is responsible for mapping an input argument to a
    /// ComponentBatch.
    ///
    /// There are default implementations for standard containers of components, as well as single
    /// components. These can be found in `rerun/component_batch_adapter_builtins.hpp`.
    ///
    /// An adapter may choose to either produce a owned or borrowed component batch.
    /// Borrowed component batches required that a pointer to the passed in ("adapted") data
    /// outlives the component batch. Owned component batches on the other hand take ownership by
    /// allocating a std::vector and moving the data into it. This is typically only required when
    /// passing in temporary objects into an adapter or non-trivial data conversion is necessary.
    ///
    /// By implementing your own adapters for certain component types, you can map your data to
    /// Rerun types which then can be logged.
    ///
    /// To implement an adapter for a type T, specialize `ComponentBatchAdapter<TComponent, T>` and
    /// define `ComponentBatch<TComponent> operator()(const T& input)`.
    /// It is *highly recommended* to also specify `ComponentBatch<TComponent> operator()(T&&
    /// input)` in order to to accidentally borrow data that is passed in as a temporary!
    ///
    /// TODO(andreas): Point to an example here and in the assert.
    template <typename TComponent, typename TInput, typename Enable = std::enable_if_t<true>>
    struct ComponentBatchAdapter {
        template <typename... Ts>
        struct NoAdapterFor : std::false_type {};

        // `NoAdapterFor` always evaluates to false, but in a way that requires template instantiation.
        static_assert(
            NoAdapterFor<TComponent, TInput>::value,
            "ComponentBatchAdapter is not implemented for this type. "
            "It is implemented for for single components as well as std::vector, std::array, and "
            "c-arrays of components. "
            "You can add your own implementation by specializing "
            "ComponentBatchAdapter<TComponent, T> for a given "
            "component and your input type T."
        );
    };
} // namespace rerun
