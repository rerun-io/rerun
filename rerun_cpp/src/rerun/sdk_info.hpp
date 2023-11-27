// General information about the SDK.
#pragma once

#include "error.hpp"

namespace rerun {
    /// The Rerun C++ SDK version as a human-readable string.
    const char* version_string();

    /// Internal check whether the version reported by the rerun_c binary matches `sdk_version_string`.
    ///
    /// This method is called on various C++ API entry points, calling `Error::handle` on the return value.
    /// There is no need to call this method yourself unless you want to ensure that rerun_c binary and
    /// rerun_c header versions match ahead of time.
    Error check_binary_and_header_version_match();
} // namespace rerun
