#pragma once

#include <array>
#include <utility>
#include <vector>

#include "data_cell.hpp"
#include "result.hpp"

namespace rerun {
    // TODO: doc
    template <typename TComponent, typename... TInputArgs>
    struct ComponentBatchAdapter;

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

        /// Creates a new empty component batch.
        ///
        /// Note that logging an empty component batch is different from logging no component batch:
        /// When you log an empty component batch at an entity that already has some components of
        /// the same type, it will clear out all components of that type.
        ComponentBatch() : ownership(BatchOwnership::Borrowed) {
            storage.borrowed.data = nullptr;
            storage.borrowed.num_instances = 0;
        }

        /// Construct using a `ComponentBatchAdapter` if there's a fitting specialization.
        template <typename... Ts>
        ComponentBatch(Ts&&... args)
            : ComponentBatch(ComponentBatchAdapter<TComponent, Ts...>()(std::forward<Ts>(args)...)
              ) {}

        // TODO: why need this??
        // TODO: make into adapter. Why can't we?
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

        static ComponentBatch<TComponent> borrow(const TComponent* data, size_t num_instances) {
            ComponentBatch<TComponent> batch;
            batch.ownership = BatchOwnership::Borrowed;
            batch.storage.borrowed.data = data;
            batch.storage.borrowed.num_instances = num_instances;
            return batch;
        }

        /// Construct from a temporary std::vector.
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
                case BatchOwnership::Moved:
                    // This shouldn't happen but is well defined. We're now also moved!
                    break;
            }
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

      private:
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

    template <typename TComponent, typename... TInputArgs>
    struct ComponentBatchAdapter {
        template <typename = std::enable_if_t<std::is_constructible_v<TComponent, TInputArgs...>>>
        ComponentBatch<TComponent> operator()(TInputArgs&&... args) {
            // TODO(andreas): Optimize this to not allocate a vector, instead have space enough
            // for a single component. (maybe up to a certain size?)
            return ComponentBatch<TComponent>::take_ownership(
                {TComponent(std::forward<TInputArgs>(args)...)}
            );
        }
    };

    // TODO: too many variants here. something is wrong

    /// Adapter of reference to std::vector.
    ///
    /// *Attention*: Does *not* take ownership of the data,
    /// you need to ensure that the data outlives the component batch.
    template <typename TComponent>
    struct ComponentBatchAdapter<TComponent, std::vector<TComponent>> {
        using ComponentType = TComponent;

        ComponentBatch<TComponent> operator()(std::vector<TComponent> input) {
            return ComponentBatch<TComponent>::take_ownership(std::move(input));
        }
    };

    template <typename TComponent>
    struct ComponentBatchAdapter<TComponent, std::vector<TComponent>&> {
        using ComponentType = TComponent;

        ComponentBatch<TComponent> operator()(const std::vector<TComponent>& input) {
            return ComponentBatch<TComponent>::borrow(input.data(), input.size());
        }
    };

    template <typename TComponent>
    struct ComponentBatchAdapter<TComponent, const std::vector<TComponent>&> {
        using ComponentType = TComponent;

        ComponentBatch<TComponent> operator()(const std::vector<TComponent>& input) {
            return ComponentBatch<TComponent>::borrow(input.data(), input.size());
        }
    };

    template <typename TComponent>
    struct ComponentBatchAdapter<TComponent, std::vector<TComponent>&&> {
        using ComponentType = TComponent;

        ComponentBatch<TComponent> operator()(std::vector<TComponent>&& input) {
            return ComponentBatch<TComponent>::take_ownership(std::move(input));
        }
    };

    /// Adapter of reference to std::array.
    ///
    /// *Attention*: Does *not* take ownership of the data,
    /// you need to ensure that the data outlives the component batch.
    template <typename TComponent, size_t NumInstances>
    struct ComponentBatchAdapter<TComponent, const std::array<TComponent, NumInstances>&> {
        using ComponentType = TComponent;

        ComponentBatch<TComponent> operator()(const std::array<TComponent, NumInstances>& array) {
            return ComponentBatch<TComponent>::borrow(array.data(), NumInstances);
        }
    };

    /// Adaptor from a C-Array reference.
    ///
    /// *Attention*: Does *not* take ownership of the data,
    /// you need to ensure that the data outlives the component batch.
    template <typename TComponent, size_t NumInstances>
    struct ComponentBatchAdapter<TComponent, TComponent[NumInstances]> {
        using ComponentType = TComponent;

        ComponentBatch<TComponent> operator()(const TComponent (&array)[NumInstances]) {
            return ComponentBatch<TComponent>::borrow(array, NumInstances);
        }
    };

    template <typename TComponent>
    struct ComponentBatchAdapter<TComponent, TComponent> {
        using ComponentType = TComponent;

        ComponentBatch<TComponent> operator()(const TComponent& one_and_only) {
            return ComponentBatch<TComponent>::borrow(&one_and_only, 1);
        }

        ComponentBatch<TComponent> operator()(TComponent&& one_and_only) {
            // TODO(andreas): Optimize this to not allocate a vector, instead have space enough for
            // a single component. (maybe up to a certain size?)
            return ComponentBatch<TComponent>::take_ownership({one_and_only});
        }
    };

    /// Adapt from an initializer list.
    ///
    /// Data in initializer lists is temporary, therefore this has to take ownership.
    // template <typename TComponent>
    // struct ComponentBatchAdapter<TComponent, std::initializer_list<TComponent>> {
    //     using ComponentType = TComponent;

    //     ComponentBatch<TComponent> operator()(std::initializer_list<TComponent> data) {
    //         return ComponentBatch<TComponent>::take_ownership(std::vector<TComponent>{data});
    //     }
    // };

    // ----------------------
    // TODO: separate file for `AsComponents`
    // TODO: `AsComponents` should be implemented for archetypes
    // ----------------------

    template <typename TComponent>
    struct AsComponents {
        Result<std::vector<SerializedComponentBatch>> serialize(const TComponent& single_component
        ) const {
            const auto result = ComponentBatch<TComponent>(single_component).serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

    template <typename TComponent>
    struct AsComponents<std::vector<TComponent>> {
        Result<std::vector<SerializedComponentBatch>> serialize(
            const std::vector<TComponent>& components
        ) const {
            const auto result = ComponentBatch<TComponent>(components).serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

    template <typename TComponent, size_t NumInstances>
    struct AsComponents<std::array<TComponent, NumInstances>> {
        Result<std::vector<SerializedComponentBatch>> serialize(
            const std::array<TComponent, NumInstances>& components
        ) const {
            const auto result = ComponentBatch<TComponent>(components).serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

    template <typename TComponent, size_t NumInstances>
    struct AsComponents<TComponent[NumInstances]> {
        Result<std::vector<SerializedComponentBatch>> serialize(const TComponent (&array
        )[NumInstances]) const {
            const auto result = ComponentBatch<TComponent>().serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

    template <typename TComponent>
    struct AsComponents<ComponentBatch<TComponent>> {
        Result<std::vector<SerializedComponentBatch>> serialize(
            const ComponentBatch<TComponent>& components
        ) const {
            const auto result = components.serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

} // namespace rerun

// TODO: add tests for when things are owned or not.
