#pragma once

#include "collection.hpp"
#include "component_batch.hpp"
#include "indicator_component.hpp"
#include "loggable.hpp"

namespace rerun {
    /// The AsComponents trait is used to convert a type into a list of component batches.
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

        /// Converts the type `T` into a collection of `ComponentBatch`s.
        static Result<Collection<ComponentBatch>> as_batches(const T& archetype);
    };

    // Documenting the builtin generic `AsComponents` impls is too much clutter for the doc class overview.
    /// \cond private

    /// `AsComponents` for a `Collection<ComponentBatch>`.
    template <>
    struct AsComponents<Collection<ComponentBatch>> {
        static Result<Collection<ComponentBatch>> as_batches(Collection<ComponentBatch> components
        ) {
            return components;
        }
    };

    /// `AsComponents` for a single `ComponentBatch`.
    template <>
    struct AsComponents<ComponentBatch> {
        static Result<Collection<ComponentBatch>> as_batches(ComponentBatch components) {
            return rerun::take_ownership(std::move(components));
        }
    };

    /// `AsComponents` for a `Collection<ComponentBatch>` wrapped in a `Result`, forwarding errors for convenience.
    template <>
    struct AsComponents<Result<Collection<ComponentBatch>>> {
        static Result<Collection<ComponentBatch>> as_batches(
            Result<Collection<ComponentBatch>> components
        ) {
            RR_RETURN_NOT_OK(components.error);
            return components.value;
        }
    };

    /// `AsComponents` for a `Collection<ComponentBatch>` individually wrapped in `Result`, forwarding errors for convenience.
    template <>
    struct AsComponents<Collection<Result<ComponentBatch>>> {
        static Result<Collection<ComponentBatch>> as_batches(
            Collection<Result<ComponentBatch>> components
        ) {
            std::vector<ComponentBatch> result;
            result.reserve(components.size());
            for (auto& component : components) {
                RR_RETURN_NOT_OK(component.error);
                result.push_back(std::move(component.value));
            }
            return rerun::take_ownership(std::move(result));
        }
    };

    /// `AsComponents` for a single `ComponentBatch` wrapped in a `Result`, forwarding errors for convenience.
    template <>
    struct AsComponents<Result<ComponentBatch>> {
        static Result<Collection<ComponentBatch>> as_batches(Result<ComponentBatch> components) {
            RR_RETURN_NOT_OK(components.error);
            return rerun::take_ownership(std::move(components.value));
        }
    };

    /// \endcond
} // namespace rerun
