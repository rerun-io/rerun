#pragma once

#include <cstdint>
#include <string>

#ifdef __cpp_exceptions
#include <stdexcept>
#endif

struct rr_error;

namespace rerun {
    /// Status codes returned by the SDK as part of `Status`.
    ///
    /// Category codes are used to group errors together, but are never returned directly.
    enum class StatusCode : uint32_t {
        Ok = 0,

        // Invalid argument errors.
        _CategoryArgument = 0x000000010,
        UnexpectedNullArgument,
        InvalidStringArgument,
        InvalidRecordingStreamHandle,
        InvalidSocketAddress,
        InvalidEntityPath,

        // Recording stream errors
        _CategoryRecordingStream = 0x000000100,
        RecordingStreamCreationFailure,
        RecordingStreamSaveFailure,

        // Arrow data processing errors.
        _CategoryArrow = 0x000001000,
        ArrowIpcMessageParsingFailure,
        ArrowDataCellError,

        Unknown = 0xFFFFFFFF,
    };

    /// Callback function type for log handlers.
    using StatusLogHandler = void (*)(const class Status& status, void* userdata);

    /// Status outcome object (success or error) returned for fallible operations.
    ///
    /// Converts to `true` for success, `false` for failure.
    class [[nodiscard]] Status {
      public:
        /// Result code for the given operation.
        StatusCode code = StatusCode::Ok;

        /// Human readable description of the error.
        std::string description;

      public:
        Status() = default;

        Status(StatusCode _code, std::string _description)
            : code(_code), description(std::move(_description)) {}

        /// Construct from a C status object.
        Status(const rr_error& status);

        /// Returns true if the code is `Ok`.
        bool is_ok() const {
            return code == StatusCode::Ok;
        }

        /// Returns true if the code is not `Ok`.
        bool is_err() const {
            return code != StatusCode::Ok;
        }

        /// Sets global log handler called for `log` and `log_error_on_failure`.
        ///
        /// The default will log to stderr.
        ///
        /// @param handler The handler to call, or `nullptr` to reset to the default.
        /// @param userdata Userdata pointer that will be passed to each invocation of the handler.
        ///
        /// @see log, log_error_on_failure
        static void set_log_handler(StatusLogHandler handler, void* userdata = nullptr);

        /// Logs this status via the global log handler.
        ///
        /// @see set_log_handler
        void log() const;

        /// Logs this status if failed via the global log handler.
        ///
        /// @see set_log_handler
        void log_error_on_failure() const {
            if (is_err()) {
                log();
            }
        }

#ifdef __cpp_exceptions
        /// Throws a `std::runtime_error` if the status is not `Ok`.
        void throw_on_failure() const {
            if (is_err()) {
                throw std::runtime_error(description);
            }
        }
#endif
    };
} // namespace rerun
