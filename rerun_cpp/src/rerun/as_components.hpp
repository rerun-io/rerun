#include "component_batch.hpp"

namespace rerun {
    // TODO: general docs

    /// AsComponents for a single component.
    template <typename TComponent>
    struct AsComponents {
        Result<std::vector<SerializedComponentBatch>> serialize(const TComponent& single_component
        ) const {
            const auto result = ComponentBatch<TComponent>(single_component).serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

    /// AsComponents for a std::vector of components.
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

    /// AsComponents for an std::array of components.
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

    /// AsComponents for an c-array of components.
    template <typename TComponent, size_t NumInstances>
    struct AsComponents<TComponent[NumInstances]> {
        Result<std::vector<SerializedComponentBatch>> serialize(const TComponent (&array
        )[NumInstances]) const {
            const auto result = ComponentBatch<TComponent>().serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

    /// AsComponents for an ComponentBatch.
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
