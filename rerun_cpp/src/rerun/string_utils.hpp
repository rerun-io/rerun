
#pragma once

#include <optional>
#include <string>
#include <string_view>

struct rr_string;
struct rr_bytes;

namespace rerun {
    namespace detail {
        rr_string to_rr_string(const std::string& str);
        rr_string to_rr_string(std::string_view str);
        rr_string to_rr_string(std::optional<std::string_view> str);

        rr_bytes to_rr_bytes(std::string_view bytes);
    } // namespace detail
} // namespace rerun
