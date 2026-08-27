#pragma once

#include <cstdint>
#include <string_view>
#include <utility>
#include <vector>

struct rr_log_sink;

namespace rerun {
    /// What happens when a client connects to a gRPC server.
    enum class PlaybackBehavior {
        /// Start by playing back all old data, then send data that arrived during playback.
        OldestFirst,

        /// Prioritize newly arriving messages, then replay history from newest to oldest.
        NewestFirst,
    };

    struct LogSink;

    /// Log sink which streams messages to an existing Rerun gRPC server.
    ///
    /// This is a gRPC client: it connects to a server but does not host one.
    /// Use `GrpcServerSink` to host a server that SDKs and Viewers can connect to.
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

    /// Log sink which hosts a Rerun gRPC server.
    ///
    /// This is a gRPC server: SDKs and Viewers connect to it.
    /// Use `GrpcSink` to connect as a client to an existing server.
    ///
    /// `RecordingStream::set_sinks` copies this configuration into a native sink.
    /// Destroying this descriptor has no effect on the server.
    /// Replacing the sinks or destroying the recording shuts the server down.
    struct GrpcServerSink {
        std::string_view bind_ip = "0.0.0.0";
        uint16_t port = 9876;
        std::string_view server_memory_limit = "1GiB";
        PlaybackBehavior playback_behavior = PlaybackBehavior::OldestFirst;
        std::vector<std::string_view> cors_allow_origins;

        GrpcServerSink() = default;

        GrpcServerSink(
            std::string_view bind_ip_, uint16_t port_ = 9876,
            std::string_view server_memory_limit_ = "1GiB",
            PlaybackBehavior playback_behavior_ = PlaybackBehavior::OldestFirst,
            std::vector<std::string_view> cors_allow_origins_ = {}
        )
            : bind_ip(bind_ip_),
              port(port_),
              server_memory_limit(server_memory_limit_),
              playback_behavior(playback_behavior_),
              cors_allow_origins(std::move(cors_allow_origins_)) {}

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
    /// * `GrpcServerSink`
    /// * `FileSink`
    struct LogSink {
        enum class Kind {
            Grpc = 0,
            File = 1,
            GrpcServer = 2,
        };

        Kind kind;

        union {
            GrpcSink grpc;
            FileSink file;

            /// Borrowed only during `RecordingStream::set_sinks`.
            ///
            /// `GrpcServerSink` contains a non-trivial `std::vector`, so storing it directly in
            /// this tagged union would require custom construction and destruction.
            /// The C bridge copies every field and does not retain this pointer.
            const GrpcServerSink* grpc_server;
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

    inline GrpcServerSink::operator LogSink() const {
        LogSink sink{};
        sink.kind = LogSink::Kind::GrpcServer;
        sink.grpc_server = this;
        return sink;
    }

    namespace detail {
        rr_log_sink to_rr_log_sink(LogSink sink);
    }
} // namespace rerun
