#pragma once

#include <type_traits> // std::false_type

namespace rerun {
    /// The Loggable trait is used by all built-in implementation of `rerun::AsComponents`
    /// to serialize a collection for logging.
    ///
    /// It is implemented for all built-in `rerun::component`s and `rerun::datatype`s.
    template <typename T>
    struct Loggable {
        /// \private
        /// `NoLoggableFor` always evaluates to false, but in a way that requires template instantiation.
        template <typename T2>
        struct NoLoggableFor : std::false_type {};

        static_assert(
            NoLoggableFor<T>::value,
            "Loggable is not implemented for this type. "
            "It is implemented for all built-in datatypes and components. "
            "To check ahead of template instantiation whether a type is loggable, use `is_loggable<T>`"
        );

        // TODO(andreas): List methods that the trait should implement.
    };

    /// \private
    namespace detail {
        /// Helper to check whether a type implements the Loggable trait.
        ///
        /// Uses SFINAE to ensure that a given type T *specializes* the `rerun::Loggable` trait.
        /// The non-specialized `Loggable<T>` is the only implementation which contains `NoLoggableFor<T>`,
        /// so if that type exists we know the type does not specialize the `Loggable` trait.
        template <typename T>
        constexpr auto is_loggable(int = 0) ->
            typename Loggable<T>::template NoLoggableFor<T>::value_type {
            return false;
        }

        /// Helper to check whether a type implements the Loggable trait.
        ///
        /// Uses SFINAE to ensure that a given type T *specializes* the `rerun::Loggable` trait.
        /// The non-specialized `Loggable<T>` is the only implementation which contains `NoLoggableFor<T>`,
        /// so if that type exists we know the type does not specialize the `Loggable` trait.
        template <typename T>
        constexpr bool is_loggable(...) {
            return true;
        }
    } // namespace detail

    /// True for any type that implements the Loggable trait.
    template <typename T>
    constexpr bool is_loggable = detail::is_loggable<T>();
} // namespace rerun
