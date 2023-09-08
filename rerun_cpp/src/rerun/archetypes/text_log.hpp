// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

#pragma once

#include "../arrow.hpp"
#include "../components/text.hpp"
#include "../components/text_log_level.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// A log entry in a text log, comprised of a text body and its log level.
        struct TextLog {
            rerun::components::Text body;

            std::optional<rerun::components::TextLogLevel> level;

          public:
            TextLog() = default;

            TextLog(rerun::components::Text _body) : body(std::move(_body)) {}

            TextLog& with_level(rerun::components::TextLogLevel _level) {
                level = std::move(_level);
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
