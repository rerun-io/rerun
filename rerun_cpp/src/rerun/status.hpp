#pragma once

#include <cstdint>
#include <string>

struct rr_status;

namespace rerun {
    /// Status codes returned by the SDK as part of `Status`.
    ///
    /// Category codes are used to group errors together, but are never returned directly.
    enum class StatusCode : uint32_t {
        Ok = 0,

        _CategoryArgument = 0x000000010,
        UnexpectedNullArgument,
        InvalidStringArgument,

        _CategoryRecordingStream = 0x000000100,
        RecordingStreamCreationFailure,

        Unknown = 0xFFFFFFFF,
    };

    /// Status outcome object (success or error) returned for fallible operations.
    ///
    /// Converts to `true` for success, `false` for failure.
    class Status {
      public:
        /// Result code for the given operation.
        StatusCode code = StatusCode::Ok;

        /// Human readable description of the error.
        std::string description;

      public:
        /// Construct from a C status object.
        Status(const rr_status& status);

        operator bool() const {
            return code != StatusCode::Ok;
        }

#ifdef __cpp_exceptions
        /// Throws a `std::runtime_error` if the status is not `Ok`.
        void throw_on_failure() const {
            if (!*this) {
                throw std::runtime_error(description);
            }
        }
#endif
    };
} // namespace rerun
