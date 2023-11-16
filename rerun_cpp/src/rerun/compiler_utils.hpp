#pragma once

// Macro for enabling and disabling the "-Wmaybe-uninitialized" warning in GCC.
// See: https://github.com/rerun-io/rerun/issues/4027

#define RERUN_WITH_MAYBE_UNINITIALIZED_DISABLED(expr) \
    RERUN_DISABLE_MAYBE_UNINITIALIZED_PUSH            \
    expr RERUN_DISABLE_MAYBE_UNINITIALIZED_POP

#if defined(__GNUC__) && !defined(__clang__)
#define RERUN_DISABLE_MAYBE_UNINITIALIZED_PUSH \
    _Pragma("GCC diagnostic push") _Pragma("GCC diagnostic ignored \"-Wmaybe-uninitialized\"")
#else
#define RERUN_DISABLE_MAYBE_UNINITIALIZED_PUSH
#endif

#if defined(__GNUC__) && !defined(__clang__)
#define RERUN_DISABLE_MAYBE_UNINITIALIZED_POP _Pragma("GCC diagnostic pop")
#else
#define RERUN_DISABLE_MAYBE_UNINITIALIZED_POP
#endif

// Macro for marking code as unreachable.
// Reaching the code after all is undefined behavior.

#if defined(__GNUC__) || defined(__clang__)
#define RERUN_UNREACHABLE() __builtin_unreachable()
#elif defined(_MSC_VER)
#define RERUN_UNREACHABLE() __assume(false)
#else
#define RERUN_UNREACHABLE() \
    do {                    \
    } while (false)
#endif
