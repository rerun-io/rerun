// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_document.fbs".

#include "text_document.hpp"

#include "../components/text.hpp"

namespace rerun {
    namespace archetypes {
        Result<std::vector<rerun::DataCell>> TextDocument::to_data_cells() const {
            std::vector<rerun::DataCell> cells;
            cells.reserve(1);

            {
                const auto result = rerun::components::Text::to_data_cell(&body, 1);
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }
            {
                const auto result = create_indicator_component(
                    "rerun.components.TextDocumentIndicator",
                    num_instances()
                );
                if (result.is_err()) {
                    return result.error;
                }
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
