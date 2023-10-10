#pragma once

#include <array>
#include <utility>
#include <vector>

#include "result.hpp"
#include "serialized_component_batch.hpp"

namespace rerun {
    /// The ComponentBatchAdaptor trait is responsible for mapping a list of input arguments to a
    /// ComponentBatch.
    ///
    /// There are default implementations for standard containers of components, as well as single
    /// components.
    ///
    /// An adapter may choose to either produce a owned or borrowed component batch.
    /// Borrowed component batches required that a pointer to the passed in ("adapted") data
    /// outlives the component batch. Owned component batches on the other hand take ownership by
    /// allocating a std::vector and moving the data into it. This is typically only required when
    /// passing in temporary objects into an adapter.
    ///
    /// By implementing your own adapters for certain component types, you can map your data to
    /// Rerun types which then can be logged.
    /// TODO(andreas): Point to an example.
    template <typename TComponent, typename... TInputArgs>
    struct ComponentBatchAdapter {
        template <typename... Ts>
        struct NoAdapterFor : std::false_type {};

        // TODO(andreas): This should also mention an example of how to implement this.
        static_assert(
            NoAdapterFor<TComponent, TInputArgs...>::value,
            "ComponentBatchAdapter is not implemented for this type. "
            "It is implemented for for single components as well as std::vector, std::array, and "
            "c-arrays of components. "
            "You can add your own implementation by specializing "
            "ComponentBatchAdapter<TComponent, T> for a given "
            "component and your input type T."
        );
    };

    /// Type of ownership of the the batch's data.
    ///
    /// User access to this is typically only needed for debugging and testing.
    enum class BatchOwnership {
        /// The component batch does not own the data and only has a pointer and a
        /// size.
        Borrowed,

        /// The component batch owns the data via an std::vector.
        VectorOwned,

        /// The component batch was moved.
        /// This could be achieved by other means, but this makes it easiest to follow and
        /// debug.
        Moved,
    };

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
        using TComponentType = TComponent;

        /// Type of an adapter given input types Ts.
        ///
        /// This type may not exist for all combinations of Ts.
        template <typename... Ts>
        using TAdapter =
            ComponentBatchAdapter<TComponent, std::remove_cv_t<std::remove_reference_t<Ts>>...>;

        /// Creates a new empty component batch.
        ///
        /// Note that logging an empty component batch is different from logging no component
        /// batch: When you log an empty component batch at an entity that already has some
        /// components of the same type, it will clear out all components of that type.
        ComponentBatch() : ownership(BatchOwnership::Borrowed) {
            storage.borrowed.data = nullptr;
            storage.borrowed.num_instances = 0;
        }

        /// Construct using a `ComponentBatchAdapter` if there's a fitting specialization.
        template <typename... Ts>
        ComponentBatch(Ts&&... args)
            : ComponentBatch(TAdapter<Ts...>()(std::forward<Ts>(args)...)) {}

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
        ComponentBatch(ComponentBatch<TComponent>&& other) {
            switch (other.ownership) {
                case BatchOwnership::Borrowed:
                    storage.borrowed = other.storage.borrowed;
                    break;
                case BatchOwnership::VectorOwned:
                    // Don't assign, since the vector is in an undefined state and assigning may
                    // attempt to free data.
                    new (&storage.vector_owned)
                        std::vector<TComponent>(std::move(other.storage.vector_owned));
                    break;
                case BatchOwnership::Moved:
                    // This shouldn't happen but is well defined. We're now also moved!
                    break;
            }
            ownership = other.ownership;
            other.ownership = BatchOwnership::Moved;
        }

        /// Move assignment
        void operator=(ComponentBatch<TComponent>&& other) {
            this->~ComponentBatch();
            new (this) ComponentBatch(std::move(other));
        }

        ~ComponentBatch() {
            switch (ownership) {
                case BatchOwnership::Moved:
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
                case BatchOwnership::Moved:
                    return 0;
            }
        }

        /// Serializes the component batch into a rerun datacell that can be sent to a store.
        Result<SerializedComponentBatch> serialize() const {
            // TODO(andreas): `to_data_cell` should actually get our storage representation passed
            // in which we'll allow "type adaptors" in the future (for e.g. setting a stride or
            // similar).
            // TODO(andreas): For improved error messages we should add a static_assert that
            // TComponent implements `to_data_cell`.
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

                case BatchOwnership::Moved:
                    return Error(
                        ErrorCode::InvalidOperationOnMovedObject,
                        "ComponentBatch was already moved and is now invalid."
                    );
            }
        }

        /// Returns the ownership of the component batch.
        ///
        /// This is usually only needed for debugging and testing.
        BatchOwnership get_ownership() const {
            return ownership;
        }

      private:
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
