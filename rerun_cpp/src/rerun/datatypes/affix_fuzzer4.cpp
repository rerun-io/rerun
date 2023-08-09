// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#include "affix_fuzzer4.hpp"

#include "../datatypes/affix_fuzzer3.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &AffixFuzzer4::to_arrow_datatype() {
            static const auto datatype = arrow::dense_union({
                arrow::field("_null_markers", arrow::null(), true, nullptr),
                arrow::field(
                    "single_required",
                    rerun::datatypes::AffixFuzzer3::to_arrow_datatype(),
                    false
                ),
                arrow::field(
                    "many_required",
                    arrow::list(arrow::field(
                        "item",
                        rerun::datatypes::AffixFuzzer3::to_arrow_datatype(),
                        false
                    )),
                    false
                ),
                arrow::field(
                    "many_optional",
                    arrow::list(arrow::field(
                        "item",
                        rerun::datatypes::AffixFuzzer3::to_arrow_datatype(),
                        false
                    )),
                    true
                ),
            });
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::DenseUnionBuilder>>
            AffixFuzzer4::new_arrow_array_builder(arrow::MemoryPool *memory_pool) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::DenseUnionBuilder>(
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    std::make_shared<arrow::NullBuilder>(memory_pool),
                    rerun::datatypes::AffixFuzzer3::new_arrow_array_builder(memory_pool)
                        .ValueOrDie(),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        rerun::datatypes::AffixFuzzer3::new_arrow_array_builder(memory_pool)
                            .ValueOrDie()
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        rerun::datatypes::AffixFuzzer3::new_arrow_array_builder(memory_pool)
                            .ValueOrDie()
                    ),
                }),
                to_arrow_datatype()
            ));
        }

        arrow::Status AffixFuzzer4::fill_arrow_array_builder(
            arrow::DenseUnionBuilder *builder, const AffixFuzzer4 *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            ARROW_RETURN_NOT_OK(builder->Reserve(num_elements));
            for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto &union_instance = elements[elem_idx];
                ARROW_RETURN_NOT_OK(builder->Append(static_cast<uint8_t>(union_instance._tag)));

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
                        ARROW_RETURN_NOT_OK(
                            rerun::datatypes::AffixFuzzer3::fill_arrow_array_builder(
                                variant_builder,
                                &union_instance._data.single_required,
                                1
                            )
                        );
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_required: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        return arrow::Status::NotImplemented(
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_optional: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        return arrow::Status::NotImplemented(
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                }
            }

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rerun
