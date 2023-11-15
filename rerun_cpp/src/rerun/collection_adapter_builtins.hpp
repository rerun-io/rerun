#pragma once

#include "collection.hpp"
#include "collection_adapter.hpp"
#include "type_traits.hpp"

// Documenting the builtin adapters is too much clutter for the doc class overview.
/// \cond private

namespace rerun {
    /// Adapter from `std::vector` of elements.
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

    /// Adapter from a generic std compatible container (see `rerun::is_iterable_and_has_size_v`) which
    /// has a value type from which `TElement` can be constructed.
    ///
    /// Since this needs to do a conversion, this will always need to allocate space.
    /// However, if a temporary is passed, elements will be moved instead of copied upon construction of `TElement`.
    template <typename TElement, typename TContainer>
    struct CollectionAdapter<
        TElement, TContainer,
        std::enable_if_t<
            !std::is_same_v<TElement, value_type_of_t<TContainer>> && //
            is_iterable_and_has_size_v<TContainer> &&                 //
            std::is_constructible_v<
                TElement,
                value_type_of_t<TContainer>> //
            >> {
        Collection<TElement> operator()(const TContainer& input) {
            std::vector<TElement> transformed;
            transformed.reserve(std::size(input));
            for (const auto& element : input) {
                transformed.emplace_back(element);
            }
            return Collection<TElement>::take_ownership(std::move(transformed));
        }

        Collection<TElement> operator()(TContainer&& input) {
            std::vector<TElement> transformed;
            transformed.reserve(std::size(input));
            for (auto& element : input) {
                transformed.emplace_back(std::move(element));
            }

            return Collection<TElement>::take_ownership(std::move(transformed));
        }
    };

    /// Adapter from std::array of components.
    ///
    /// Only takes ownership if a temporary is passed.
    /// TODO(andreas): change this to adapt anything that takes data() and size() but isn't a vector
    /// (vectors are special since we can take ownership directly)
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
    /// Only takes ownership if a temporary is passed, borrows otherwise.
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

    /// Adapter for a single element from which `TElement`, temporary or reference.
    ///
    /// Only takes ownership if a temporary is passed, borrows otherwise.
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
