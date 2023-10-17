// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#include "affix_fuzzer4.hpp"

#include "affix_fuzzer3.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &AffixFuzzer4::arrow_datatype() {
            static const auto datatype = arrow::dense_union({
                arrow::field("_null_markers", arrow::null(), true, nullptr),
                arrow::field(
                    "single_required",
                    rerun::datatypes::AffixFuzzer3::arrow_datatype(),
                    false
                ),
                arrow::field(
                    "many_required",
                    arrow::list(arrow::field(
                        "item",
                        rerun::datatypes::AffixFuzzer3::arrow_datatype(),
                        false
                    )),
                    false
                ),
                arrow::field(
                    "many_optional",
                    arrow::list(arrow::field(
                        "item",
                        rerun::datatypes::AffixFuzzer3::arrow_datatype(),
                        false
                    )),
                    true
                ),
            });
            return datatype;
        }

        Result<std::shared_ptr<arrow::DenseUnionBuilder>> AffixFuzzer4::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::DenseUnionBuilder>(
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    std::make_shared<arrow::NullBuilder>(memory_pool),
                    rerun::datatypes::AffixFuzzer3::new_arrow_array_builder(memory_pool).value,
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        rerun::datatypes::AffixFuzzer3::new_arrow_array_builder(memory_pool).value
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        rerun::datatypes::AffixFuzzer3::new_arrow_array_builder(memory_pool).value
                    ),
                }),
                arrow_datatype()
            ));
        }

        Error AffixFuzzer4::fill_arrow_array_builder(
            arrow::DenseUnionBuilder *builder, const AffixFuzzer4 *elements, size_t num_elements
        ) {
            if (!builder) {
                return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
            }
            if (!elements) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto &union_instance = elements[elem_idx];
                ARROW_RETURN_NOT_OK(builder->Append(static_cast<int8_t>(union_instance._tag)));

                auto variant_index = static_cast<int>(union_instance._tag);
                auto variant_builder_untyped = builder->child_builder(variant_index).get();

                switch (union_instance._tag) {
                    case detail::AffixFuzzer4Tag::NONE: {
                        ARROW_RETURN_NOT_OK(variant_builder_untyped->AppendNull());
                        break;
                    }
                    case detail::AffixFuzzer4Tag::single_required: {
                        auto variant_builder =
                            static_cast<arrow::DenseUnionBuilder *>(variant_builder_untyped);
                        RR_RETURN_NOT_OK(rerun::datatypes::AffixFuzzer3::fill_arrow_array_builder(
                            variant_builder,
                            &union_instance._data.single_required,
                            1
                        ));
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_required: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "Failed to serialize AffixFuzzer4::many_required: list types in unions not yet implemented"
                        );
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_optional: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "Failed to serialize AffixFuzzer4::many_optional: list types in unions not yet implemented"
                        );
                        break;
                    }
                }
            }

            return Error::ok();
        }
    } // namespace datatypes
} // namespace rerun
