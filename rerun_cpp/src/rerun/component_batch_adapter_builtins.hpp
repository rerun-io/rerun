#pragma once

#include "component_batch.hpp"
#include "component_batch_adapter.hpp"

namespace rerun {
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
    struct ComponentBatchAdapter<
        TComponent, std::vector<T>,
        std::enable_if_t<
            !std::is_same_v<TComponent, T> && std::is_constructible_v<TComponent, const T&>>> {
        ComponentBatch<TComponent> operator()(const std::vector<T>& input) {
            std::vector<TComponent> transformed(input.size());

            std::transform(input.begin(), input.end(), transformed.begin(), [](const T& datum) {
                return TComponent(datum);
            });

            return ComponentBatch<TComponent>::take_ownership(std::move(transformed));
        }

        ComponentBatch<TComponent> operator()(std::vector<T>&& input) {
            std::vector<TComponent> transformed(input.size());

            std::transform(
                std::make_move_iterator(input.begin()),
                std::make_move_iterator(input.end()),
                transformed.begin(),
                [](T&& datum) { return TComponent(std::move(datum)); }
            );

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

    /// Adapter from a C-Array reference.
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
