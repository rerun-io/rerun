#include "log_sink.hpp"
#include "string_utils.hpp"
#include "c/rerun.h"

#include <cassert>

namespace rerun {
    namespace detail {
        rr_log_sink to_rr_log_sink(LogSink sink) {
            rr_log_sink out;
            switch (sink.kind) {
                case LogSink::Kind::Grpc:
                    out.kind = RR_LOG_SINK_KIND_GRPC;
                    out.grpc = rr_grpc_sink{
                        detail::to_rr_string(sink.grpc.url),
                        sink.grpc.flush_timeout_sec,
                    };
                    break;
                case LogSink::Kind::File:
                    out.kind = RR_LOG_SINK_KIND_FILE;
                    out.file = rr_file_sink{detail::to_rr_string(sink.file.path)};
                    break;
                default:
                    assert(false && "unreachable");
            }
            return out;
        }
    }
}
