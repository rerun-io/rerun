// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#include "../datatypes/affix_fuzzer3.hpp"
#include "affix_fuzzer15.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<components::AffixFuzzer15>::arrow_datatype() {
        static const auto datatype = Loggable<rerun::datatypes::AffixFuzzer3>::arrow_datatype();
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<components::AffixFuzzer15>::to_arrow(
        const components::AffixFuzzer15* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<components::AffixFuzzer15>::fill_arrow_array_builder(
                static_cast<arrow::DenseUnionBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    rerun::Error Loggable<components::AffixFuzzer15>::fill_arrow_array_builder(
        arrow::DenseUnionBuilder* builder, const components::AffixFuzzer15* elements,
        size_t num_elements
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
} // namespace rerun
