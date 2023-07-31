// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer6.hpp"

#include "../datatypes/affix_fuzzer1.hpp"
#include "../rerun.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        const char* AffixFuzzer6::NAME = "rerun.testing.components.AffixFuzzer6";

        const std::shared_ptr<arrow::DataType>& AffixFuzzer6::to_arrow_datatype() {
            static const auto datatype = rr::datatypes::AffixFuzzer1::to_arrow_datatype();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>> AffixFuzzer6::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(
                rr::datatypes::AffixFuzzer1::new_arrow_array_builder(memory_pool).ValueOrDie()
            );
        }

        arrow::Status AffixFuzzer6::fill_arrow_array_builder(
            arrow::StructBuilder* builder, const AffixFuzzer6* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            return arrow::Status::NotImplemented(("TODO(andreas) Handle nullable extensions"));

            return arrow::Status::OK();
        }

        arrow::Result<rr::DataCell> AffixFuzzer6::to_data_cell(
            const AffixFuzzer6* components, size_t num_components
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, AffixFuzzer6::new_arrow_array_builder(pool));
            if (components && num_components > 0) {
                ARROW_RETURN_NOT_OK(AffixFuzzer6::fill_arrow_array_builder(
                    builder.get(),
                    components,
                    num_components
                ));
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer6::NAME, AffixFuzzer6::to_arrow_datatype(), false)}
            );

            rr::DataCell cell;
            cell.component_name = AffixFuzzer6::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rr::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rr
