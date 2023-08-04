// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/archetypes/annotation_context.fbs"

#include "annotation_context.hpp"

#include "../components/annotation_context.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace archetypes {
        arrow::Result<std::vector<rerun::DataCell>> AnnotationContext::to_data_cells() const {
            std::vector<rerun::DataCell> cells;
            cells.reserve(1);

            {
                ARROW_ASSIGN_OR_RAISE(
                    const auto cell,
                    rerun::components::AnnotationContext::to_data_cell(&context, 1)
                );
                cells.push_back(cell);
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
