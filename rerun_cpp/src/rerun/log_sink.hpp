#pragma once

#include <string_view>

struct rr_log_sink;

namespace rerun {
    struct LogSink;

    /// Log sink which streams messages to a gRPC server.
    ///
    /// The behavior of this sink is the same as the one set by `RecordingStream::connect_grpc`.
    struct GrpcSink {
        /// A Rerun gRPC URL.
        ///
        /// The scheme must be one of `rerun://`, `rerun+http://`, or `rerun+https://`,
        /// and the pathname must be `/proxy`.
        ///
        /// The default is `rerun+http://127.0.0.1:9876/proxy`.
        std::string_view url = "rerun+http://127.0.0.1:9876/proxy";

        inline operator LogSink() const;
    };

    /// Log sink which writes messages to a file.
    struct FileSink {
        /// Path to the output file.
        std::string_view path;

        inline operator LogSink() const;
    };

    /// A sink for log messages.
    ///
    /// See specific log sink types for more information:
    /// * `GrpcSink`
    /// * `FileSink`
    struct LogSink {
        enum class Kind {
            Grpc = 0,
            File = 1,
        };

        Kind kind;

        union {
            GrpcSink grpc;
            FileSink file;
        };
    };

    inline GrpcSink::operator LogSink() const {
        LogSink sink{};
        sink.kind = LogSink::Kind::Grpc;
        sink.grpc = *this;
        return sink;
    }

    inline FileSink::operator LogSink() const {
        LogSink sink{};
        sink.kind = LogSink::Kind::File;
        sink.file = *this;
        return sink;
    }

    namespace detail {
        rr_log_sink to_rr_log_sink(LogSink sink);
    }
} // namespace rerun
