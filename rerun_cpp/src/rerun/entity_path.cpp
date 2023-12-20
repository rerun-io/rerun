#include "entity_path.hpp"

#include "c/rerun.h"
#include "error.hpp"
#include "string_utils.hpp"

namespace rerun {
    std::string escape_entity_path_part(std::string_view unescaped) {
        auto escaped_c_str = _rr_escape_entity_path_part(detail::to_rr_string(unescaped));

        if (escaped_c_str == nullptr) {
            Error(ErrorCode::InvalidStringArgument, "Failed to escape entity path part").handle();
            return std::string(unescaped);
        } else {
            std::string result = escaped_c_str;
            _rr_free_string(escaped_c_str);
            return result;
        }
    }

    std::string new_entity_path(const std::vector<std::string_view>& path) {
        if (path.empty()) {
            return "/";
        }

        std::string result;

        for (const auto& part : path) {
            result += "/";
            result += escape_entity_path_part(part);
        }

        return result;
    }

} // namespace rerun
