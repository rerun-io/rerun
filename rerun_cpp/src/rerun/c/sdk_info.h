/// Returns the version of the Rerun C SDK.
///
/// This should match the string returned by `rr_version_string` (C) or `rerun::version_string` (C++).
/// If not, the SDK's binary and the C header are out of sync.
#define RERUN_SDK_HEADER_VERSION "0.22.0-alpha.1+dev"

/// Major version of the Rerun C SDK.
#define RERUN_SDK_HEADER_VERSION_MAJOR 0

/// Minor version of the Rerun C SDK.
#define RERUN_SDK_HEADER_VERSION_MINOR 22

/// Patch version of the Rerun C SDK.
#define RERUN_SDK_HEADER_VERSION_PATCH 0

/// Is the Rerun library version greater or equal to this?
///
/// Example usage:
/// ```
/// #if RERUN_VERSION_GE(0, 18, 0)
///    // Use features from Rerun 0.18
/// #endif
/// ```
#define RERUN_VERSION_GE(major, minor, patch)                                                      \
    ((major) == RERUN_SDK_HEADER_VERSION_MAJOR                                                     \
         ? ((minor) == RERUN_SDK_HEADER_VERSION_MINOR ? (patch) <= RERUN_SDK_HEADER_VERSION_PATCH  \
                                                      : (minor) <= RERUN_SDK_HEADER_VERSION_MINOR) \
         : (major) <= RERUN_SDK_HEADER_VERSION_MAJOR)
