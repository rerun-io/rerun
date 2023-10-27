#pragma once

// Macro for enabling and disabling the "-Wmaybe-uninitialized" warning in GCC.
// See: https://github.com/rerun-io/rerun/issues/4027

#define WITH_MAYBE_UNINITIALIZED_DISABLED(expr) \
    DISABLE_MAYBE_UNINITIALIZED_PUSH            \
    expr DISABLE_MAYBE_UNINITIALIZED_POP

#if defined(__GNUC__) && !defined(__clang__)
#define DISABLE_MAYBE_UNINITIALIZED_PUSH \
    _Pragma("GCC diagnostic push") _Pragma("GCC diagnostic ignored \"-Wmaybe-uninitialized\"")
#else
#define DISABLE_MAYBE_UNINITIALIZED_PUSH
#endif

#if defined(__GNUC__) && !defined(__clang__)
#define DISABLE_MAYBE_UNINITIALIZED_POP _Pragma("GCC diagnostic pop")
#else
#define DISABLE_MAYBE_UNINITIALIZED_POP
#endif
