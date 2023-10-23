#pragma once

#include <algorithm>
#include <array>
#include <utility>
#include <vector>

#include "result.hpp"
#include "serialized_component_batch.hpp"

namespace rerun {
    /// The ComponentBatchAdaptor trait is responsible for mapping an input argument to a
    /// ComponentBatch.
    ///
    /// There are default implementations for standard containers of components, as well as single
    /// components.
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
    template <typename TComponent, typename TInput>
    struct ComponentBatchAdapter {
        template <typename... Ts>
        struct NoAdapterFor : std::false_type {};

        static_assert(
            NoAdapterFor<TComponent, TInput>::value, // Always evaluate to false, but in a way that
                                                     // requires template instantiation.
            "ComponentBatchAdapter is not implemented for this type. "
            "It is implemented for for single components as well as std::vector, std::array, and "
            "c-arrays of components. "
            "You can add your own implementation by specializing "
            "ComponentBatchAdapter<TComponent, T> for a given "
            "component and your input type T."
        );
    };

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
        using TAdapter =
            ComponentBatchAdapter<TComponent, std::remove_cv_t<std::remove_reference_t<T>>>;

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

        /// Borrows data into the component batch.
        ///
        /// Borrowed data must outlive the component batch!
        /// (If the pointer passed is into an std::vector or similar, this std::vector mustn't be
        /// resized.)
        static ComponentBatch<TComponent> borrow(const TComponent* data, size_t num_instances) {
            ComponentBatch<TComponent> batch;
            batch.ownership = BatchOwnership::Borrowed;
            batch.storage.borrowed.data = data;
            batch.storage.borrowed.num_instances = num_instances;
            return batch;
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
            this->swap(other);
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

            ComponentBatchStorage() {}

            ~ComponentBatchStorage() {}
        };

        BatchOwnership ownership;
        ComponentBatchStorage<TComponent> storage;
    };

    /// Adapter from std::vector of components.
    ///
    /// Only takes ownership if a temporary is passed.
    template <typename TComponent>
    struct ComponentBatchAdapter<TComponent, std::vector<TComponent>> {
        ComponentBatch<TComponent> operator()(const std::vector<TComponent>& input) {
            return ComponentBatch<TComponent>::borrow(input.data(), input.size());
        }

        ComponentBatch<TComponent> operator()(std::vector<TComponent>&& input) {
            return ComponentBatch<TComponent>::take_ownership(std::move(input));
        }
    };

    /// Adapter from std::vector<T> where T can be converted to TComponent
    template <typename TComponent, typename T>
    struct ComponentBatchAdapter<TComponent, std::vector<T>> {
        ComponentBatch<TComponent> operator()(const std::vector<T>& input) {
            std::vector<TComponent> transformed(input.size());

            std::transform(input.begin(), input.end(), transformed.begin(), [](auto datum) {
                return TComponent(datum);
            });

            return ComponentBatch<TComponent>::take_ownership(std::move(transformed));
        }

        ComponentBatch<TComponent> operator()(std::vector<T>&& input) {
            std::vector<TComponent> transformed(input.size());

            std::transform(input.begin(), input.end(), transformed.begin(), [](auto datum) {
                return TComponent(datum);
            });

            return ComponentBatch<TComponent>::take_ownership(std::move(transformed));
        }
    };

    /// Adapter from std::array of components.
    ///
    /// Only takes ownership if a temporary is passed.
    template <typename TComponent, size_t NumInstances>
    struct ComponentBatchAdapter<TComponent, std::array<TComponent, NumInstances>> {
        ComponentBatch<TComponent> operator()(const std::array<TComponent, NumInstances>& array) {
            return ComponentBatch<TComponent>::borrow(array.data(), NumInstances);
        }

        ComponentBatch<TComponent> operator()(std::array<TComponent, NumInstances>&& array) {
            return ComponentBatch<TComponent>::take_ownership(
                std::vector<TComponent>(array.begin(), array.end())
            );
        }
    };

    /// Adaptor from a C-Array reference.
    ///
    /// *Attention*: Does *not* take ownership of the data,
    /// you need to ensure that the data outlives the component batch.
    template <typename TComponent, size_t NumInstances>
    struct ComponentBatchAdapter<TComponent, TComponent[NumInstances]> {
        ComponentBatch<TComponent> operator()(const TComponent (&array)[NumInstances]) {
            return ComponentBatch<TComponent>::borrow(array, NumInstances);
        }

        ComponentBatch<TComponent> operator()(TComponent (&&array)[NumInstances]) {
            std::vector<TComponent> components;
            components.reserve(NumInstances);
            components.insert(
                components.end(),
                std::make_move_iterator(array),
                std::make_move_iterator(array + NumInstances)
            );
            return ComponentBatch<TComponent>::take_ownership(std::move(components));
        }
    };

    /// Adapter for a single component, temporary or reference.
    template <typename TComponent>
    struct ComponentBatchAdapter<TComponent, TComponent> {
        ComponentBatch<TComponent> operator()(const TComponent& one_and_only) {
            return ComponentBatch<TComponent>::borrow(&one_and_only, 1);
        }

        ComponentBatch<TComponent> operator()(TComponent&& one_and_only) {
            return ComponentBatch<TComponent>::take_ownership(std::move(one_and_only));
        }
    };
} // namespace rerun
