#include "spawn.hpp"
#include "c/rerun.h"
#include "config.hpp"

namespace rerun {
    Error spawn(const SpawnOptions& options) {
        RR_RETURN_NOT_OK(check_binary_and_header_version_match());

        rr_spawn_options rerun_c_options = {};
        options.fill_rerun_c_struct(rerun_c_options);
        rr_error error = {};
        rr_spawn(&rerun_c_options, &error);
        return Error(error);
    }
} // namespace rerun
