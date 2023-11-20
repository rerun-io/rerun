#pragma once

#include <iterator> // std::begin, std::end, std::size
#include <type_traits>

/// Type trait utilities.
///
/// The defined traits acts as an extension to std defined type traits and are used as utilities
/// across the SDK.
namespace rerun::traits {
    /// Gets the value/element type of a container.
    ///
    /// This works for all types that stick with the std convention of having a `value_type` member type.
    /// Fails to compile if the type does not have a `value_type` member type - this can be used for SFINAE checks.
    template <typename T>
    using value_type_of_t = typename std::remove_reference_t<T>::value_type;

    /// \private
    namespace details {
        /// False type if a given type is not iterable and has a size (has `begin` and `end`).
        template <typename T, typename = void>
        struct is_iterable : std::false_type {};

        /// True type if a given type is iterable and has a size (has `begin` and `end` implemented).
        ///
        /// Makes no restrictions on the type returned by `begin`/`end`.
        template <typename T>
        struct is_iterable<
            T, std::void_t<
                   decltype(std::begin(std::declval<T&>())), //
                   decltype(std::end(std::declval<T&>()))    //
                   >> : std::true_type {};
    } // namespace details

    /// True if a given type is iterable, meaning there is a `begin` & `end` implementation.
    ///
    /// Makes no restrictions on the type returned by `begin`/`end`.
    template <typename T>
    constexpr bool is_iterable_v = details::is_iterable<T>::value;
} // namespace rerun::traits
