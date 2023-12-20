#include "entity_path.hpp"

#include "c/rerun.h"
#include "error.hpp"
#include "string_utils.hpp"

namespace rerun {
    std::string new_entity_path(const std::vector<std::string_view>& path) {
        if (path.empty()) {
            return "/";
        }

        std::string result;

        for (const auto& part : path) {
            auto escaped_c_str = _rr_escape_entity_path_part(detail::to_rr_string(part));

            if (escaped_c_str == nullptr) {
                Error(ErrorCode::InvalidStringArgument, "Failed to escape entity path part")
                    .handle();
            } else {
                if (!result.empty()) {
                    result += "/"; // leading slash would also have be fine
                }
                result += escaped_c_str;
                _rr_free_string(escaped_c_str);
            }
        }

        return result;
    }

} // namespace rerun
