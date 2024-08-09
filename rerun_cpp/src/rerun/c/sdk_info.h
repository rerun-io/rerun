/// Returns the version of the Rerun C SDK.
///
/// This should match the string returned by `rr_version_string` (C) or `rerun::version_string` (C++).
/// If not, the SDK's binary and the C header are out of sync.
#define RERUN_SDK_HEADER_VERSION "0.18.0-alpha.1"

/// Major version of the Rerun C SDK.
#define RERUN_SDK_HEADER_VERSION_MAJOR 0

/// Minor version of the Rerun C SDK.
#define RERUN_SDK_HEADER_VERSION_MINOR 18

/// Patch version of the Rerun C SDK.
#define RERUN_SDK_HEADER_VERSION_PATCH 0
