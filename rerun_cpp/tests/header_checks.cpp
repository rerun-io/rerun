#include <rerun.hpp>

// ARROW_EXPORT is included by <arrow/util/visibility.h>
// ARROW_EXPAND is included by <arrow/util/macros.h>
// Both are included by almost all arrow headers.
#if defined(ARROW_EXPORT) || defined(ARROW_EXPAND)
static_assert(
    false,
    "ARROW_EXPORT or ARROW_EXPAND should not be defined. This indicates that we're leaking arrow "
    "headers through "
    "rerun.hpp!"
);
#endif

#if defined(RERUN_H)
static_assert(
    false,
    "RERUN_H should not be defined. This indicates that we're leaking the c/rerun.h "
    "through "
    "rerun.hpp!"
);
#endif
