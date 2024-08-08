#pragma once

// Push pop warnings
#if defined(__GNUC__) || defined(__clang__)
#define RR_PUSH_WARNINGS _Pragma("GCC diagnostic push")
#define RR_POP_WARNINGS _Pragma("GCC diagnostic pop")
#elif defined(_MSC_VER)
#define RR_PUSH_WARNINGS __pragma(warning(push))
#define RR_POP_WARNINGS __pragma(warning(pop))
#else
#define RR_PUSH_WARNINGS
#define RR_POP_WARNINGS
#endif

// Macro for enabling and disabling the "-Wmaybe-uninitialized" warning in GCC.
// See: https://github.com/rerun-io/rerun/issues/4027

#define RR_WITH_MAYBE_UNINITIALIZED_DISABLED(expr) \
    RR_DISABLE_MAYBE_UNINITIALIZED_PUSH            \
    expr RR_DISABLE_MAYBE_UNINITIALIZED_POP

#if defined(__GNUC__) && !defined(__clang__)

#define RR_DISABLE_MAYBE_UNINITIALIZED_PUSH \
    RR_PUSH_WARNINGS                        \
    _Pragma("GCC diagnostic ignored \"-Wmaybe-uninitialized\"")
#else
#define RR_DISABLE_MAYBE_UNINITIALIZED_PUSH RR_PUSH_WARNINGS
#endif

#define RR_DISABLE_MAYBE_UNINITIALIZED_POP RR_POP_WARNINGS

// Macro for marking code as unreachable.
// Reaching the code after all is undefined behavior.

#if defined(__GNUC__) || defined(__clang__)
#define RR_UNREACHABLE() __builtin_unreachable()
#elif defined(_MSC_VER)
#define RR_UNREACHABLE() __assume(false)
#else
#define RR_UNREACHABLE() \
    do {                 \
    } while (false)
#endif

// Disable deprecation warning
#if defined(__GNUC__) || defined(__clang__)
#define RR_DISABLE_DEPRECATION_WARNING \
    _Pragma("GCC diagnostic ignored \"-Wdeprecated-declarations\"")
#elif defined(_MSC_VER)
#define RR_DISABLE_DEPRECATION_WARNING __pragma(warning(disable : 4996))
#else
#define RR_DISABLE_DEPRECATION_WARNING
#endif

// TODO: needed?
// Detecting address sanitizer (ASAN) being enabled.
// #if defined(__clang__)
// #define RR_ASAN_ENABLED __has_feature(address_sanitizer)
// #else
// // Both GCC and MSVC 2019 use this macro
// // MSVC: https://learn.microsoft.com/en-us/cpp/sanitizers/asan-building?view=msvc-160#__sanitize_address__
// // GCC: https://gcc.gnu.org/onlinedocs/cpp/Common-Predefined-Macros.html
// #define RR_ASAN_ENABLED (defined(__SANITIZE_ADDRESS__) && __SANITIZE_ADDRESS__)
// #endif

// // Disable address sanitizer (ASAN) for a function.
// #if RR_ASAN_ENABLED
// #if defined(_MSC_VER)
// __declspec(no_sanitize_address)
// #else
// #define RR_DISABLE_ADDRESS_SANITIZER __attribute__((no_sanitize("address")))
// #endif
// #else
// #define RR_DISABLE_ADDRESS_SANITIZER
// #endif
