#pragma once

#include "collection.hpp"
#include "collection_adapter.hpp"
#include "type_traits.hpp"

#include <array>
#include <vector>

// Documenting the builtin adapters is too much clutter for the doc class overview.
/// \cond private

namespace rerun {
    /// Adapter from `std::vector` of elements with the target type.
    ///
    /// Only takes ownership if a temporary is passed.
    /// No allocation or copy is performed in any case. Furthermore, elements are not moved.
    template <typename TElement>
    struct CollectionAdapter<TElement, std::vector<TElement>> {
        Collection<TElement> operator()(const std::vector<TElement>& input) {
            return Collection<TElement>::borrow(input.data(), input.size());
        }

        Collection<TElement> operator()(std::vector<TElement>&& input) {
            return Collection<TElement>::take_ownership(std::move(input));
        }
    };

    /// Adapter for a iterable container (see `rerun::traits::is_iterable_v`) which
    /// has a value type from which `TElement` can be constructed but is not equal to `TElement`.
    ///
    /// Since this needs to do a conversion, this will always need to allocate space.
    /// However, if a temporary is passed, elements will be moved instead of copied upon construction of `TElement`.
    template <typename TElement, typename TContainer>
    struct CollectionAdapter<
        TElement, TContainer,
        std::enable_if_t<
            !std::is_same_v<TElement, traits::value_type_of_t<TContainer>> && //
            traits::is_iterable_v<TContainer> &&                              //
            std::is_constructible_v<
                TElement,
                traits::value_type_of_t<TContainer>> //
            >> {
        Collection<TElement> operator()(const TContainer& input) {
            std::vector<TElement> elements(std::begin(input), std::end(input));
            return Collection<TElement>::take_ownership(std::move(elements));
        }

        Collection<TElement> operator()(TContainer&& input) {
            std::vector<TElement> elements;
            // There's no batch emplace method, so we need to reserve and then emplace manually.
            // We decide here to take the performance cost if a the input's iterator is not a random access iterator.
            // (in that case determining the size will have linear complexity)
            elements.reserve(static_cast<size_t>(std::distance(std::begin(input), std::end(input)))
            );
            for (auto& element : input) {
                elements.emplace_back(std::move(element));
            }

            return Collection<TElement>::take_ownership(std::move(elements));
        }
    };

    /// Adapter from std::array of elements with the target type.
    ///
    /// Only takes ownership if a temporary is passed in which case an allocation and per element move is performed.
    template <typename TElement, size_t NumInstances>
    struct CollectionAdapter<TElement, std::array<TElement, NumInstances>> {
        Collection<TElement> operator()(const std::array<TElement, NumInstances>& array) {
            return Collection<TElement>::borrow(array.data(), NumInstances);
        }

        Collection<TElement> operator()(std::array<TElement, NumInstances>&& array) {
            std::vector<TElement> elements(
                std::make_move_iterator(array.begin()),
                std::make_move_iterator(array.end())
            );
            return Collection<TElement>::take_ownership(std::move(elements));
        }
    };

    /// Adapter from a C-Array reference with the target type.
    ///
    /// Only takes ownership if a temporary is passed in which case an allocation and per element move is performed.
    template <typename TElement, size_t NumInstances>
    struct CollectionAdapter<TElement, TElement[NumInstances]> {
        Collection<TElement> operator()(const TElement (&array)[NumInstances]) {
            return Collection<TElement>::borrow(array, NumInstances);
        }

        Collection<TElement> operator()(TElement (&&array)[NumInstances]) {
            std::vector<TElement> elements(
                std::make_move_iterator(array),
                std::make_move_iterator(array + NumInstances)
            );
            return Collection<TElement>::take_ownership(std::move(elements));
        }
    };

    /// Adapter for a single element from which `TElement`, temporary or reference.
    ///
    /// Only takes ownership if a temporary is passed in which case the element is moved.
    /// Otherwise a borrow takes place.
    template <typename TElement>
    struct CollectionAdapter<TElement, TElement> {
        Collection<TElement> operator()(const TElement& one_and_only) {
            return Collection<TElement>::borrow(&one_and_only, 1);
        }

        Collection<TElement> operator()(TElement&& one_and_only) {
            return Collection<TElement>::take_ownership(std::move(one_and_only));
        }
    };

    /// Adapter for a single element of from which `TElement` can be constructed.
    ///
    /// Since this needs to do a conversion, this will always need to allocate space.
    /// However, if a temporary is passed the element will be moved instead of copied upon construction of `TElement`.
    template <typename TElement, typename TInput>
    struct CollectionAdapter<
        TElement, TInput,
        std::enable_if_t<
            !std::is_same_v<TElement, TInput> &&      //
            !traits::is_iterable_v<TInput> &&         //
            std::is_constructible_v<TElement, TInput> //
            >> {
        Collection<TElement> operator()(const TInput& input) {
            return Collection<TElement>::take_ownership(TElement(input));
        }

        Collection<TElement> operator()(TInput&& input) {
            return Collection<TElement>::take_ownership(TElement(std::move(input)));
        }
    };

} // namespace rerun

/// \endcond
