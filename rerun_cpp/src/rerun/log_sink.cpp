#include "log_sink.hpp"
#include "c/rerun.h"
#include "string_utils.hpp"

#include <cassert>

namespace rerun {
    namespace detail {
        rr_log_sink to_rr_log_sink(LogSink sink) {
            switch (sink.kind) {
                case LogSink::Kind::Grpc: {
                    rr_log_sink out;
                    out.kind = RR_LOG_SINK_KIND_GRPC;
                    out.grpc = rr_grpc_sink{detail::to_rr_string(sink.grpc.url)};
                    return out;
                }
                case LogSink::Kind::File: {
                    rr_log_sink out;
                    out.kind = RR_LOG_SINK_KIND_FILE;
                    out.file = rr_file_sink{detail::to_rr_string(sink.file.path)};
                    return out;
                }
                default:
                    assert(false && "unreachable");
            }
            return {};
        }
    } // namespace detail
} // namespace rerun
