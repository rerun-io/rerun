#include "component_batch.hpp"
#include "indicator_component.hpp"

namespace rerun {
    // TODO: general docs
    // TODO: can we not put single component into the unspecialized one?

    /// AsComponents for a single component.
    template <typename TComponent>
    struct AsComponents {
        template <typename T>
        struct NoAsComponentsFor : std::false_type {};

        // TODO(andreas): This should also mention an example of how to implement this.
        static_assert(
            NoAsComponentsFor<TComponent>::value,
            "AsComponents is not implemented for this type. "
            "It is implemented for all built-in archetypes as well as std::vector, std::array, and "
            "c-arrays of components. "
            "You can add your own implementation by specializing AsComponents<T> for your type T."
        );
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
            const auto result = ComponentBatch<TComponent>(array).serialize();
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

    /// AsComponents for single indicators
    template <const char Name[]>
    struct AsComponents<components::IndicatorComponent<Name>> {
        Result<std::vector<SerializedComponentBatch>> serialize(
            const components::IndicatorComponent<Name>& indicator
        ) const {
            const auto result =
                ComponentBatch<components::IndicatorComponent<Name>>(indicator).serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

} // namespace rerun
