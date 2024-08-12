#include <rerun.hpp>

static_assert(RERUN_VERSION_GE(0, 18, 0), "Rerun version was expected to be at least 0.18.0");
static_assert(
    RERUN_VERSION_GE(
        RERUN_SDK_HEADER_VERSION_MAJOR, RERUN_SDK_HEADER_VERSION_MINOR,
        RERUN_SDK_HEADER_VERSION_PATCH
    ),
    "Rerun version is equal to this version."
);
static_assert(
    !RERUN_VERSION_GE(
        RERUN_SDK_HEADER_VERSION_MAJOR, RERUN_SDK_HEADER_VERSION_MINOR,
        RERUN_SDK_HEADER_VERSION_PATCH + 1
    ),
    "Rerun version is not greater than this version."
);
static_assert(
    !RERUN_VERSION_GE(
        RERUN_SDK_HEADER_VERSION_MAJOR, RERUN_SDK_HEADER_VERSION_MINOR + 1,
        RERUN_SDK_HEADER_VERSION_PATCH
    ),
    "Rerun version is not greater than this version."
);
static_assert(
    !RERUN_VERSION_GE(
        RERUN_SDK_HEADER_VERSION_MAJOR + 1, RERUN_SDK_HEADER_VERSION_MINOR,
        RERUN_SDK_HEADER_VERSION_PATCH
    ),
    "Rerun version is not greater than this version."
);

#if RERUN_VERSION_GE(0, 18, 0)
static_assert(true, "Rerun can be used in a macro.");
#else
static_assert(false, "Rerun can be used in a macro, but we shouldn't be able to get here.");
#endif
