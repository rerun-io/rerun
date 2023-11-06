#include "spawn.hpp"
#include "c/rerun.h"

namespace rerun {
    Error spawn(const SpawnOptions& options) {
        rr_spawn_options rerun_c_options = {};
        options.fill_rerun_c_struct(rerun_c_options);
        rr_error error = {};
        rr_spawn(&rerun_c_options, &error);
        return Error(error);
    }
} // namespace rerun
