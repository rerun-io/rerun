// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/tensor_buffer.fbs".

#include "tensor_buffer.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &TensorBuffer::arrow_datatype() {
            static const auto datatype = arrow::dense_union({
                arrow::field("_null_markers", arrow::null(), true, nullptr),
                arrow::field("U8", arrow::list(arrow::field("item", arrow::uint8(), false)), false),
                arrow::field(
                    "U16",
                    arrow::list(arrow::field("item", arrow::uint16(), false)),
                    false
                ),
                arrow::field(
                    "U32",
                    arrow::list(arrow::field("item", arrow::uint32(), false)),
                    false
                ),
                arrow::field(
                    "U64",
                    arrow::list(arrow::field("item", arrow::uint64(), false)),
                    false
                ),
                arrow::field("I8", arrow::list(arrow::field("item", arrow::int8(), false)), false),
                arrow::field(
                    "I16",
                    arrow::list(arrow::field("item", arrow::int16(), false)),
                    false
                ),
                arrow::field(
                    "I32",
                    arrow::list(arrow::field("item", arrow::int32(), false)),
                    false
                ),
                arrow::field(
                    "I64",
                    arrow::list(arrow::field("item", arrow::int64(), false)),
                    false
                ),
                arrow::field(
                    "F16",
                    arrow::list(arrow::field("item", arrow::float16(), false)),
                    false
                ),
                arrow::field(
                    "F32",
                    arrow::list(arrow::field("item", arrow::float32(), false)),
                    false
                ),
                arrow::field(
                    "F64",
                    arrow::list(arrow::field("item", arrow::float64(), false)),
                    false
                ),
                arrow::field(
                    "JPEG",
                    arrow::list(arrow::field("item", arrow::uint8(), false)),
                    false
                ),
            });
            return datatype;
        }

        Result<std::shared_ptr<arrow::DenseUnionBuilder>> TensorBuffer::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::DenseUnionBuilder>(
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    std::make_shared<arrow::NullBuilder>(memory_pool),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::UInt8Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::UInt16Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::UInt32Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::UInt64Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::Int8Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::Int16Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::Int32Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::Int64Builder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::HalfFloatBuilder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::FloatBuilder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::DoubleBuilder>(memory_pool)
                    ),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::UInt8Builder>(memory_pool)
                    ),
                }),
                arrow_datatype()
            ));
        }

        Error TensorBuffer::fill_arrow_array_builder(
            arrow::DenseUnionBuilder *builder, const TensorBuffer *elements, size_t num_elements
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
                    case detail::TensorBufferTag::NONE: {
                        ARROW_RETURN_NOT_OK(variant_builder_untyped->AppendNull());
                        break;
                    }
                    case detail::TensorBufferTag::U8: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::U16: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::U32: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::U64: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::I8: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::I16: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::I32: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::I64: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::F16: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::F32: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::F64: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        (void)variant_builder;
                        return Error(
                            ErrorCode::NotImplemented,
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::TensorBufferTag::JPEG: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
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
