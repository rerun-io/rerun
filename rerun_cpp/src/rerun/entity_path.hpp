#pragma once

#include <string_view>
#include <vector>

namespace rerun {
    /// Construct an entity path by escaping each part of the path.
    ///
    /// For instance, `rerun::new_entity_path({"world", 42, "unescaped string!"})` will return
    /// `"world/42/escaped\ string\!"`.
    std::string new_entity_path(const std::vector<std::string_view>& path);

} // namespace rerun
