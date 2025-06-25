#pragma once

#include <string_view>

struct rr_log_sink;

namespace rerun {
    struct LogSink;

    // TODO: document

    struct GrpcSink {
        std::string_view url = "rerun+http://127.0.0.1:9876/proxy";
        float flush_timeout_sec = 3.0;

        inline operator LogSink() const;
    };

    struct FileSink {
        std::string_view path;

        inline operator LogSink() const;
    };

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
        sink.grpc = GrpcSink{url, flush_timeout_sec};
        return sink;
    }

    inline FileSink::operator LogSink() const {
        LogSink sink{};
        sink.kind = LogSink::Kind::File;
        sink.file = FileSink{path};
        return sink;
    }

    namespace detail {
        rr_log_sink to_rr_log_sink(LogSink sink);
    };
}; // namespace rerun
