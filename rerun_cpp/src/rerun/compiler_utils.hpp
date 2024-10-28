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

// Disable possible null reference warning
#if defined(__GNUC__) || defined(__clang__)
#define RR_DISABLE_NULL_DEREF_WARNING _Pragma("GCC diagnostic ignored \"-Wnull-dereference\"")
#else
#define RR_DISABLE_NULL_DEREF_WARNING
#endif
