#pragma once

#include "collection.hpp"
#include "collection_adapter.hpp"

// Documenting the builtin adapters is too much clutter for the doc class overview.
/// \cond private

namespace rerun {
    /// Adapter from std::vector of components.
    ///
    /// Only takes ownership if a temporary is passed.
    template <typename TElement>
    struct CollectionAdapter<TElement, std::vector<TElement>> {
        Collection<TElement> operator()(const std::vector<TElement>& input) {
            return Collection<TElement>::borrow(input.data(), input.size());
        }

        Collection<TElement> operator()(std::vector<TElement>&& input) {
            return Collection<TElement>::take_ownership(std::move(input));
        }
    };

    /// Adapter from std::vector<T> where T can be converted to TElement
    template <typename TElement, typename T>
    struct CollectionAdapter<
        TElement, std::vector<T>,
        std::enable_if_t<
            !std::is_same_v<TElement, T> && std::is_constructible_v<TElement, const T&>>> {
        Collection<TElement> operator()(const std::vector<T>& input) {
            std::vector<TElement> transformed(input.size());

            std::transform(input.begin(), input.end(), transformed.begin(), [](const T& datum) {
                return TElement(datum);
            });

            return Collection<TElement>::take_ownership(std::move(transformed));
        }

        Collection<TElement> operator()(std::vector<T>&& input) {
            std::vector<TElement> transformed(input.size());

            std::transform(
                std::make_move_iterator(input.begin()),
                std::make_move_iterator(input.end()),
                transformed.begin(),
                [](T&& datum) { return TElement(std::move(datum)); }
            );

            return Collection<TElement>::take_ownership(std::move(transformed));
        }
    };

    /// Adapter from std::array of components.
    ///
    /// Only takes ownership if a temporary is passed.
    template <typename TElement, size_t NumInstances>
    struct CollectionAdapter<TElement, std::array<TElement, NumInstances>> {
        Collection<TElement> operator()(const std::array<TElement, NumInstances>& array) {
            return Collection<TElement>::borrow(array.data(), NumInstances);
        }

        Collection<TElement> operator()(std::array<TElement, NumInstances>&& array) {
            return Collection<TElement>::take_ownership(
                std::vector<TElement>(array.begin(), array.end())
            );
        }
    };

    /// Adapter from a C-Array reference.
    ///
    /// *Attention*: Does *not* take ownership of the data,
    /// you need to ensure that the data outlives the component batch.
    template <typename TElement, size_t NumInstances>
    struct CollectionAdapter<TElement, TElement[NumInstances]> {
        Collection<TElement> operator()(const TElement (&array)[NumInstances]) {
            return Collection<TElement>::borrow(array, NumInstances);
        }

        Collection<TElement> operator()(TElement (&&array)[NumInstances]) {
            std::vector<TElement> components;
            components.reserve(NumInstances);
            components.insert(
                components.end(),
                std::make_move_iterator(array),
                std::make_move_iterator(array + NumInstances)
            );
            return Collection<TElement>::take_ownership(std::move(components));
        }
    };

    /// Adapter for a single component, temporary or reference.
    template <typename TElement>
    struct CollectionAdapter<TElement, TElement> {
        Collection<TElement> operator()(const TElement& one_and_only) {
            return Collection<TElement>::borrow(&one_and_only, 1);
        }

        Collection<TElement> operator()(TElement&& one_and_only) {
            return Collection<TElement>::take_ownership(std::move(one_and_only));
        }
    };
} // namespace rerun

/// \endcond
