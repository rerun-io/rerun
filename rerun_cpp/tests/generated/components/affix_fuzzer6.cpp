// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#include "affix_fuzzer6.hpp"

#include "../datatypes/affix_fuzzer1.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char AffixFuzzer6::NAME[] = "rerun.testing.components.AffixFuzzer6";

    const std::shared_ptr<arrow::DataType>& AffixFuzzer6::arrow_datatype() {
        static const auto datatype = rerun::datatypes::AffixFuzzer1::arrow_datatype();
        return datatype;
    }

    rerun::Error AffixFuzzer6::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const AffixFuzzer6* elements, size_t num_elements
    ) {
        (void)builder;
        (void)elements;
        (void)num_elements;
        if (true) {
            return rerun::Error(
                ErrorCode::NotImplemented,
                "TODO(andreas) Handle nullable extensions"
            );
        }

        return Error::ok();
    }

    Result<rerun::DataCell> AffixFuzzer6::to_data_cell(
        const AffixFuzzer6* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(AffixFuzzer6::fill_arrow_array_builder(
                static_cast<arrow::StructBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(
            AffixFuzzer6::NAME,
            AffixFuzzer6::arrow_datatype(),
            std::move(array)
        );
    }
} // namespace rerun::components
