#pragma once

#include <string>
#include <string_view>
#include <vector>

namespace rerun {

    /// Escape an individual part of an entity path.
    ///
    /// For instance, `escape_entity_path_path("my image!")` will return `"my\ image\!"`.
    std::string escape_entity_path_part(std::string_view str);

    /// Construct an entity path by escaping each part of the path.
    ///
    /// For instance, `rerun::new_entity_path({"world", 42, "unescaped string!"})` will return
    /// `"world/42/escaped\ string\!"`.
    std::string new_entity_path(const std::vector<std::string_view>& path);

} // namespace rerun
