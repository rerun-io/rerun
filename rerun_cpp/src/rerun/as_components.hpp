#pragma once

#include "component_batch.hpp"
#include "indicator_component.hpp"

namespace rerun {
    /// The AsComponents trait is used to convert a type into a list of serialized component.
    ///
    /// It is implemented for various built-in types as well as collections of components.
    /// You can build your own archetypes by implementing this trait.
    /// Anything that implements `AsComponents` can be logged to a recording stream.
    template <typename T>
    struct AsComponents {
        template <typename T2>
        struct NoAsComponentsFor : std::false_type {};

        // TODO(andreas): This should also mention an example of how to implement this.
        static_assert(
            NoAsComponentsFor<T>::value, // Always evaluate to false, but in a way that requires
                                         // template instantiation.
            "AsComponents is not implemented for this type. "
            "It is implemented for all built-in archetypes as well as std::vector, std::array, and "
            "c-arrays of components. "
            "You can add your own implementation by specializing AsComponents<T> for your type T."
        );
    };

    /// AsComponents for a ComponentBatch.
    template <typename TComponent>
    struct AsComponents<ComponentBatch<TComponent>> {
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const ComponentBatch<TComponent>& components
        ) {
            const auto result = components.serialize();
            RR_RETURN_NOT_OK(result.error);
            return Result(std::vector<SerializedComponentBatch>{std::move(result.value)});
        }
    };

    /// AsComponents for a std::vector of components.
    template <typename TComponent>
    struct AsComponents<std::vector<TComponent>> {
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const std::vector<TComponent>& components
        ) {
            return AsComponents<ComponentBatch<TComponent>>::serialize(components);
        }
    };

    /// AsComponents for an std::array of components.
    template <typename TComponent, size_t NumInstances>
    struct AsComponents<std::array<TComponent, NumInstances>> {
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const std::array<TComponent, NumInstances>& components
        ) {
            return AsComponents<ComponentBatch<TComponent>>::serialize(components);
        }
    };

    /// AsComponents for an c-array of components.
    template <typename TComponent, size_t NumInstances>
    struct AsComponents<TComponent[NumInstances]> {
        static Result<std::vector<SerializedComponentBatch>> serialize(const TComponent (&components
        )[NumInstances]) {
            return AsComponents<ComponentBatch<TComponent>>::serialize(components);
        }
    };

    /// AsComponents for single indicators
    template <const char Name[]>
    struct AsComponents<components::IndicatorComponent<Name>> {
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const components::IndicatorComponent<Name>& indicator
        ) {
            return AsComponents<ComponentBatch<components::IndicatorComponent<Name>>>::serialize(
                indicator
            );
        }
    };

} // namespace rerun
