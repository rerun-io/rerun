#include "string_utils.hpp"

#include "c/rerun.h"

#include <string>

namespace rerun {
    namespace detail {
        rr_string to_rr_string(const std::string& str) {
            return to_rr_string(std::string_view(str));
        }

        rr_string to_rr_string(std::string_view str) {
            rr_string result;
            result.utf8 = str.data();
            result.length_in_bytes = static_cast<uint32_t>(str.length());
            return result;
        }

        rr_string to_rr_string(std::optional<std::string_view> str) {
            if (str.has_value()) {
                return to_rr_string(str.value());
            } else {
                rr_string result;
                result.utf8 = nullptr;
                result.length_in_bytes = 0;
                return result;
            }
        }

        rr_bytes to_rr_bytes(std::string_view bytes) {
            rr_bytes result;
            result.bytes = bytes.data();
            result.length = static_cast<uint32_t>(bytes.length());
            return result;
        }
    } // namespace detail
} // namespace rerun
