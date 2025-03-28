#pragma once

#include <cstdint>
#include <string_view>

extern "C" struct rr_spawn_options;

namespace rerun {

    /// Options to control the behavior of `spawn`.
    ///
    /// Refer to the field-level documentation for more information about each individual options.
    ///
    /// The defaults are ok for most use cases.
    ///
    /// Keep this in sync with rerun.h's `rr_spawn_options`.
    struct SpawnOptions {
        /// The port to listen on.
        uint16_t port = 9876;

        /// An upper limit on how much memory the Rerun Viewer should use.
        ///
        /// When this limit is reached, Rerun will drop the oldest data.
        /// Example: `16GB` or `50%` (of system total).
        ///
        /// Defaults to `75%` if unset.
        std::string_view memory_limit = "75%";

        /// Hide the normal Rerun welcome screen.
        ///
        /// Defaults to `false` if unset.
        bool hide_welcome_screen = false;

        /// Detach Rerun Viewer process from the application process.
        ///
        /// Defaults to `true` if unset.
        bool detach_process = true;

        /// Specifies the name of the Rerun executable.
        ///
        /// You can omit the `.exe` suffix on Windows.
        ///
        /// Defaults to `rerun` if unset.
        std::string_view executable_name = "rerun";

        /// Enforce a specific executable to use instead of searching though PATH
        /// for `SpawnOptions::executable_name`.
        std::string_view executable_path;

        /// Convert to the corresponding rerun_c struct for internal use.
        ///
        /// _Implementation note:_
        /// By not returning it we avoid including the C header in this header.
        /// \private
        void fill_rerun_c_struct(rr_spawn_options& spawn_opts) const;
    };
} // namespace rerun
