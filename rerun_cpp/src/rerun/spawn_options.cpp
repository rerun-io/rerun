#include "spawn_options.hpp"
#include "c/rerun.h"
#include "string_utils.hpp"

namespace rerun {
    void SpawnOptions::fill_rerun_c_struct(rr_spawn_options& spawn_opts) const {
        spawn_opts.port = port;
        spawn_opts.memory_limit = detail::to_rr_string(memory_limit);
        spawn_opts.server_memory_limit = detail::to_rr_string(server_memory_limit);
        spawn_opts.hide_welcome_screen = hide_welcome_screen;
        spawn_opts.detach_process = detach_process;
        spawn_opts.executable_name = detail::to_rr_string(executable_name);
        spawn_opts.executable_path = detail::to_rr_string(executable_path);
    }
} // namespace rerun
