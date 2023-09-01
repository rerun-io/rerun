// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#include "affix_fuzzer3.hpp"

#include "affix_fuzzer1.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &AffixFuzzer3::arrow_datatype() {
            static const auto datatype = arrow::dense_union({
                arrow::field("_null_markers", arrow::null(), true, nullptr),
                arrow::field("degrees", arrow::float32(), false),
                arrow::field("radians", arrow::float32(), true),
                arrow::field(
                    "craziness",
                    arrow::list(arrow::field(
                        "item",
                        rerun::datatypes::AffixFuzzer1::arrow_datatype(),
                        false
                    )),
                    false
                ),
                arrow::field(
                    "fixed_size_shenanigans",
                    arrow::fixed_size_list(arrow::field("item", arrow::float32(), false), 3),
                    false
                ),
            });
            return datatype;
        }

        Result<std::shared_ptr<arrow::DenseUnionBuilder>> AffixFuzzer3::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::DenseUnionBuilder>(
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    std::make_shared<arrow::NullBuilder>(memory_pool),
                    std::make_shared<arrow::FloatBuilder>(memory_pool),
                    std::make_shared<arrow::FloatBuilder>(memory_pool),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        rerun::datatypes::AffixFuzzer1::new_arrow_array_builder(memory_pool).value
                    ),
                    std::make_shared<arrow::FixedSizeListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::FloatBuilder>(memory_pool),
                        3
                    ),
                }),
                arrow_datatype()
            ));
        }

        Error AffixFuzzer3::fill_arrow_array_builder(
            arrow::DenseUnionBuilder *builder, const AffixFuzzer3 *elements, size_t num_elements
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
                    case detail::AffixFuzzer3Tag::NONE: {
                        ARROW_RETURN_NOT_OK(variant_builder_untyped->AppendNull());
                        break;
                    }
                    case detail::AffixFuzzer3Tag::degrees: {
                        auto variant_builder =
                            static_cast<arrow::FloatBuilder *>(variant_builder_untyped);
                        ARROW_RETURN_NOT_OK(variant_builder->Append(union_instance._data.degrees));
                        break;
                    }
                    case detail::AffixFuzzer3Tag::radians: {
                        auto variant_builder =
                            static_cast<arrow::FloatBuilder *>(variant_builder_untyped);
                        const auto &element = union_instance._data;
                        if (element.radians.has_value()) {
                            ARROW_RETURN_NOT_OK(variant_builder->Append(element.radians.value()));
                        } else {
                            ARROW_RETURN_NOT_OK(variant_builder->AppendNull());
                        }
                        break;
                    }
                    case detail::AffixFuzzer3Tag::craziness: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::AffixFuzzer3Tag::fixed_size_shenanigans: {
                        auto variant_builder =
                            static_cast<arrow::FixedSizeListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                }
            }

            return Error::ok();
        }
    } // namespace datatypes
} // namespace rerun
