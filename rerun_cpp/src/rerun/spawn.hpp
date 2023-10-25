// General information about the SDK.
#pragma once

#include <cstdint> // uint32_t etc.

#include "error.hpp"

namespace rerun {
    /// Spawns a new Rerun Viewer process from an executable available in PATH, ready to
    /// listen for incoming TCP connections.
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
    ///
    /// flush_timeout_sec:
    /// The minimum time the SDK will wait during a flush before potentially
    /// dropping data if progress is not being made. Passing a negative value indicates no
    /// timeout, and can cause a call to `flush` to block indefinitely.
    Error spawn(
        uint16_t port = 9876,                  //
        const char* memory_limit = "75%",      //
        const char* executable_name = nullptr, //
        const char* executable_path = nullptr, //
        float flush_timeout_sec = 2.0          //
    );
} // namespace rerun
