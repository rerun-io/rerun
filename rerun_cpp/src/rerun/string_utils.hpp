
#pragma once

#include <optional>
#include <string>
#include <string_view>

struct rr_string;

namespace rerun {
    namespace detail {
        rr_string to_rr_string(const std::string& str);
        rr_string to_rr_string(std::string_view str);
        rr_string to_rr_string(std::optional<std::string_view> str);
    } // namespace detail
} // namespace rerun
