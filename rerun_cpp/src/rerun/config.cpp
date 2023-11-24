#include "config.hpp"

#include <algorithm>
#include <cstdlib>
#include <string>

#include "c/rerun.h"

namespace rerun {

    RerunGlobalConfig& RerunGlobalConfig::instance() {
        static RerunGlobalConfig global;
        return global;
    }

    RerunGlobalConfig::RerunGlobalConfig() : default_enabled(true) {}

    const char* const sdk_version_string = RERUN_SDK_HEADER_VERSION;

    Error check_binary_and_header_version_match() {
        const char* binary_version = rr_version_string();

        if (strcmp(binary_version, sdk_version_string) == 0) {
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
                    .append(sdk_version_string)
            );
        }
    }

} // namespace rerun
