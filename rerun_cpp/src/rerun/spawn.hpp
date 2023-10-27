// General information about the SDK.
#pragma once

#include <cstdint> // uint32_t etc.
#include <optional>
#include <string_view>

#include "error.hpp"

namespace rerun {
    /// Spawns a new Rerun Viewer process from an executable available in PATH, ready to
    /// listen for incoming TCP connections.
    ///
    /// If a Rerun Viewer is already listening on this TCP port, the stream will be redirected to
    /// that viewer instead of starting a new one.
    ///
    /// ## Parameters
    ///
    /// port:
    /// The port to listen on.
    ///
    /// memory_limit:
    /// An upper limit on how much memory the Rerun Viewer should use.
    /// When this limit is reached, Rerun will drop the oldest data.
    /// Example: `16GB` or `50%` (of system total).
    ///
    /// executable_name:
    /// Specifies the name of the Rerun executable.
    /// You can omit the `.exe` suffix on Windows.
    ///
    /// executable_path:
    /// Enforce a specific executable to use instead of searching though PATH
    /// for [`Self::executable_name`].
    Error spawn(
        uint16_t port = 9876,                                                //
        const std::string_view memory_limit = "75%",                         //
        const std::string_view executable_name = "rerun",                    //
        const std::optional<std::string_view> executable_path = std::nullopt //
    );
} // namespace rerun
