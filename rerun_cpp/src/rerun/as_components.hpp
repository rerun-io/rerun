#pragma once

#include "collection.hpp"
#include "component_batch.hpp"
#include "indicator_component.hpp"
#include "loggable.hpp"

namespace rerun {
    /// The AsComponents trait is used to convert a type into a list of serialized component.
    ///
    /// It is implemented for various built-in types as well as collections of components.
    /// You can build your own archetypes by implementing this trait.
    /// Anything that implements `AsComponents` can be logged to a recording stream.
    template <typename T>
    struct AsComponents {
        /// \private
        /// `NoAsComponentsFor` always evaluates to false, but in a way that requires template instantiation.
        template <typename T2>
        struct NoAsComponentsFor : std::false_type {};

        // TODO(andreas): This should also mention an example of how to implement this.
        static_assert(
            NoAsComponentsFor<T>::value,
            "AsComponents is not implemented for this type. "
            "It is implemented for all built-in archetypes as well as invidiual & collections of `rerun::ComponentBatch`."
            "You can add your own implementation by specializing AsComponents<T> for your type T."
        );

        // TODO(andreas): List methods that the trait should implement.
    };

    // Documenting the builtin generic `AsComponents` impls is too much clutter for the doc class overview.
    /// \cond private

    /// `AsComponents` for a Collection of types implementing the `rerun::Loggable` trait.

    /// `AsComponents` for a `Collection<ComponentBatch>`.
    template <>
    struct AsComponents<Collection<ComponentBatch>> {
        static Result<std::vector<ComponentBatch>> serialize(Collection<ComponentBatch> components
        ) {
            return Result<std::vector<ComponentBatch>>(std::move(components).to_vector());
        }
    };

    /// `AsComponents` for a single `ComponentBatch`.
    template <>
    struct AsComponents<ComponentBatch> {
        static Result<std::vector<ComponentBatch>> serialize(ComponentBatch components) {
            return Result<std::vector<ComponentBatch>>({std::move(components)});
        }
    };

    /// `AsComponents` for a `Collection<ComponentBatch>` wrapped in a `Result`, forwarding errors for convenience.
    template <>
    struct AsComponents<Result<Collection<ComponentBatch>>> {
        static Result<std::vector<ComponentBatch>> serialize(
            Result<Collection<ComponentBatch>> components
        ) {
            RR_RETURN_NOT_OK(components.error);
            return Result<std::vector<ComponentBatch>>(std::move(components.value).to_vector());
        }
    };

    /// `AsComponents` for a single `ComponentBatch` wrapped in a `Result`, forwarding errors for convenience.
    template <>
    struct AsComponents<Result<ComponentBatch>> {
        static Result<std::vector<ComponentBatch>> serialize(Result<ComponentBatch> components) {
            RR_RETURN_NOT_OK(components.error);
            return Result<std::vector<ComponentBatch>>({std::move(components.value)});
        }
    };

    template <typename TComponent>
    [[deprecated(
        "Direct serialization of component collections is deprecated. Either use archetype constructors or construct `ComponentBatch` with explicit descriptors."
    )]] struct AsComponents<Collection<TComponent>> {
        static_assert(
            is_loggable<TComponent>, "The given type does not implement the rerun::Loggable trait."
        );

        static Result<std::vector<ComponentBatch>> serialize(
            const Collection<TComponent>& components
        ) {
            auto batch_result = ComponentBatch::from_loggable<TComponent>(components);
            RR_RETURN_NOT_OK(batch_result.error);

            return Result<std::vector<ComponentBatch>>({std::move(batch_result.value)});
        }
    };

    /// `AsComponents` for a `std::vector` of types implementing the `rerun::Loggable` trait.
    template <typename TComponent>
    [[deprecated(
        "Direct serialization of component collections is deprecated. Either use archetype constructors or construct `ComponentBatch` with explicit descriptors."
    )]] struct AsComponents<std::vector<TComponent>> {
        static Result<std::vector<ComponentBatch>> serialize(
            const std::vector<TComponent>& components
        ) {
            return AsComponents<Collection<TComponent>>::serialize(components);
        }
    };

    /// AsComponents for `std::initializer_list`
    template <typename TComponent>
    [[deprecated(
        "Direct serialization of component collections is deprecated. Either use archetype constructors or construct `ComponentBatch` with explicit descriptors."
    )]] struct AsComponents<std::initializer_list<TComponent>> {
        static Result<std::vector<ComponentBatch>> serialize(
            std::initializer_list<TComponent> components
        ) {
            return AsComponents<Collection<TComponent>>::serialize(components);
        }
    };

    /// `AsComponents` for an `std::array` of types implementing the `rerun::Loggable` trait.
    template <typename TComponent, size_t NumInstances>
    [[deprecated(
        "Direct serialization of component collections is deprecated. Either use archetype constructors or construct `ComponentBatch` with explicit descriptors."
    )]] struct AsComponents<std::array<TComponent, NumInstances>> {
        static Result<std::vector<ComponentBatch>> serialize(
            const std::array<TComponent, NumInstances>& components
        ) {
            return AsComponents<Collection<TComponent>>::serialize(components);
        }
    };

    /// `AsComponents` for an c-array of types implementing the `rerun::Loggable` trait.
    template <typename TComponent, size_t NumInstances>
    [[deprecated(
        "Direct serialization of component collections is deprecated. Either use archetype constructors or construct `ComponentBatch` with explicit descriptors."
    )]] struct AsComponents<TComponent[NumInstances]> {
        static Result<std::vector<ComponentBatch>> serialize(const TComponent (&components
        )[NumInstances]) {
            return AsComponents<Collection<TComponent>>::serialize(components);
        }
    };

    /// `AsComponents` for single indicator components.
    template <const char ComponentName[]>
    [[deprecated(
        "Direct serialization of component collections is deprecated. Either use archetype constructors or construct `ComponentBatch` with explicit descriptors."
    )]] struct AsComponents<components::IndicatorComponent<ComponentName>> {
        static Result<std::vector<ComponentBatch>> serialize(
            const components::IndicatorComponent<ComponentName>& indicator
        ) {
            return AsComponents<
                Collection<components::IndicatorComponent<ComponentName>>>::serialize(indicator);
        }
    };

    /// \endcond
} // namespace rerun
