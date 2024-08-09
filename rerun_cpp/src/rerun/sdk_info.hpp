// General information about the SDK.
#pragma once

#include "c/sdk_info.h"
#include "error.hpp"

namespace rerun {
    /// Returns a human-readable version string of the Rerun C SDK.
    ///
    /// This should match the string in `RERUN_SDK_HEADER_VERSION`.
    /// If not, the SDK's binary and the C++ headers are out of sync.
    const char* version_string();

    /// Internal check whether the version reported by the rerun_c binary matches `sdk_version_string`.
    ///
    /// This method is called on various C++ API entry points, calling `Error::handle` on the return value.
    /// There is no need to call this method yourself unless you want to ensure that rerun_c binary and
    /// rerun_c header versions match ahead of time.
    Error check_binary_and_header_version_match();
} // namespace rerun
