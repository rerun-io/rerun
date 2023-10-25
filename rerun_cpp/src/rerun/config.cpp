#include "config.hpp"

#include <algorithm>
#include <cstdlib>
#include <string>

namespace rerun {

    RerunGlobalConfig& RerunGlobalConfig::instance() {
        static RerunGlobalConfig global;
        return global;
    }

    RerunGlobalConfig::RerunGlobalConfig() {
        // NOTE: we set the default value of default_enabled to match the environment
        // variable, but in reality this doesn't actually do anything. The rust SDK
        // will independently parse RERUN and disable `RecordingStream`s if it's set.
        const char* envVarValue = std::getenv("RERUN");
        if (envVarValue != nullptr) {
            std::string envVarValueStr(envVarValue);
            std::transform(
                envVarValueStr.begin(),
                envVarValueStr.end(),
                envVarValueStr.begin(),
                ::tolower
            );
            if (envVarValueStr == "1" || envVarValueStr == "true" || envVarValueStr == "yes") {
                default_enabled.store(true, std::memory_order_seq_cst);
            } else if (envVarValueStr == "0" || envVarValueStr == "false" || envVarValueStr == "no") {
                default_enabled.store(false, std::memory_order_seq_cst);
            }
        } else {
            default_enabled = true;
        }
    }

} // namespace rerun
