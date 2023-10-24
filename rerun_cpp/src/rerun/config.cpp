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
#if RERUN_ENABLED
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
                enabled.store(true, std::memory_order_seq_cst);
            } else if (envVarValueStr == "0" || envVarValueStr == "false" || envVarValueStr == "no") {
                enabled.store(false, std::memory_order_seq_cst);
            }
        } else {
            enabled = true;
        }
#else
        enabled = false;
#endif
    }

} // namespace rerun
