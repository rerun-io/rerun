#include <iterator> // std::begin, std::end, std::size
#include <type_traits>

namespace rerun {
    /// Gets the value/element type of a container.
    ///
    /// This works for all types that stick with the std convention of having a `value_type` member type.
    template <typename T>
    using value_type_of_t = typename std::remove_reference_t<T>::value_type;

    /// \private
    namespace details {
        /// \private
        template <typename T, typename = void>
        struct is_iterable_and_has_size : std::false_type {};

        /// \private
        template <typename T>
        struct is_iterable_and_has_size<
            T, std::void_t<
                   decltype(std::begin(std::declval<T&>())), //
                   decltype(std::end(std::declval<T&>())),   //
                   decltype(std::size(std::declval<T&>()))   //
                   >> : std::true_type {};
    } // namespace details

    /// True if a given type is iterable (has `begin` & `end`) and has a `size` member function.
    ///
    /// Makes no restrictions on the type returned by `begin`/`end`/`size`.
    template <typename T>
    constexpr bool is_iterable_and_has_size_v = details::is_iterable_and_has_size<T>::value;
} // namespace rerun
