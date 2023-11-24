#include "sdk_info.hpp"
#include "c/rerun.h"

#include <cstring> // strcmp
#include <string>

#include "c/rerun.h"

namespace rerun {
    const char* version_string() {
        return rr_version_string();
    }

    Error check_binary_and_header_version_match() {
        const char* binary_version = version_string();

        if (strcmp(binary_version, RERUN_SDK_HEADER_VERSION) == 0) {
            return Error();
        } else {
            return Error(
                ErrorCode::SdkVersionMismatch,
                std::string(
                    "Rerun_c SDK version and SDK header/source versions don't match. "
                    "Make sure to link against the correct version of the rerun_c library.\n"
                    "Rerun_c version:\n"
                )
                    .append(binary_version)
                    .append("\nSDK header/source version:\n")
                    .append(RERUN_SDK_HEADER_VERSION)
            );
        }
    }
} // namespace rerun
