// DO NOT EDIT!: This file was autogenerated by re_types_builder in
// crates/re_types_builder/src/codegen/cpp/mod.rs:54 Based on
// "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#pragma once

#include "affix_fuzzer3.hpp"

#include <cstdint>
#include <cstring>
#include <memory>
#include <new>
#include <optional>
#include <rerun/result.hpp>
#include <utility>
#include <vector>

namespace arrow {
    class DataType;
    class DenseUnionBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        namespace detail {
            enum class AffixFuzzer4Tag : uint8_t {
                /// Having a special empty state makes it possible to implement move-semantics. We
                /// need to be able to leave the object in a state which we can run the destructor
                /// on.
                NONE = 0,
                single_required,
                many_required,
                many_optional,
            };

            union AffixFuzzer4Data {
                rerun::datatypes::AffixFuzzer3 single_required;

                std::vector<rerun::datatypes::AffixFuzzer3> many_required;

                std::optional<std::vector<rerun::datatypes::AffixFuzzer3>> many_optional;

                AffixFuzzer4Data() {}

                ~AffixFuzzer4Data() {}

                void swap(AffixFuzzer4Data &other) noexcept {
                    // This bitwise swap would fail for self-referential types, but we don't have
                    // any of those.
                    char temp[sizeof(AffixFuzzer4Data)];
                    void *otherbytes = reinterpret_cast<void *>(&other);
                    void *thisbytes = reinterpret_cast<void *>(this);
                    std::memcpy(temp, thisbytes, sizeof(AffixFuzzer4Data));
                    std::memcpy(thisbytes, otherbytes, sizeof(AffixFuzzer4Data));
                    std::memcpy(otherbytes, temp, sizeof(AffixFuzzer4Data));
                }
            };
        } // namespace detail

        struct AffixFuzzer4 {
            AffixFuzzer4() : _tag(detail::AffixFuzzer4Tag::NONE) {}

            AffixFuzzer4(const AffixFuzzer4 &other) : _tag(other._tag) {
                switch (other._tag) {
                    case detail::AffixFuzzer4Tag::single_required: {
                        _data.single_required = other._data.single_required;
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_required: {
                        _data.many_required = other._data.many_required;
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_optional: {
                        _data.many_optional = other._data.many_optional;
                        break;
                    }
                    case detail::AffixFuzzer4Tag::NONE:
                        const void *otherbytes = reinterpret_cast<const void *>(&other._data);
                        void *thisbytes = reinterpret_cast<void *>(&this->_data);
                        std::memcpy(thisbytes, otherbytes, sizeof(detail::AffixFuzzer4Data));
                        break;
                }
            }

            AffixFuzzer4 &operator=(const AffixFuzzer4 &other) noexcept {
                AffixFuzzer4 tmp(other);
                this->swap(tmp);
                return *this;
            }

            AffixFuzzer4(AffixFuzzer4 &&other) noexcept : _tag(detail::AffixFuzzer4Tag::NONE) {
                this->swap(other);
            }

            AffixFuzzer4 &operator=(AffixFuzzer4 &&other) noexcept {
                this->swap(other);
                return *this;
            }

            ~AffixFuzzer4() {
                switch (this->_tag) {
                    case detail::AffixFuzzer4Tag::NONE: {
                        break; // Nothing to destroy
                    }
                    case detail::AffixFuzzer4Tag::single_required: {
                        typedef rerun::datatypes::AffixFuzzer3 TypeAlias;
                        _data.single_required.~TypeAlias();
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_required: {
                        typedef std::vector<rerun::datatypes::AffixFuzzer3> TypeAlias;
                        _data.many_required.~TypeAlias();
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_optional: {
                        typedef std::optional<std::vector<rerun::datatypes::AffixFuzzer3>>
                            TypeAlias;
                        _data.many_optional.~TypeAlias();
                        break;
                    }
                }
            }

            void swap(AffixFuzzer4 &other) noexcept {
                auto tag_temp = this->_tag;
                this->_tag = other._tag;
                other._tag = tag_temp;
                this->_data.swap(other._data);
            }

            static AffixFuzzer4 single_required(rerun::datatypes::AffixFuzzer3 single_required) {
                typedef rerun::datatypes::AffixFuzzer3 TypeAlias;
                AffixFuzzer4 self;
                self._tag = detail::AffixFuzzer4Tag::single_required;
                new (&self._data.single_required) TypeAlias(std::move(single_required));
                return self;
            }

            static AffixFuzzer4 many_required(
                std::vector<rerun::datatypes::AffixFuzzer3> many_required
            ) {
                typedef std::vector<rerun::datatypes::AffixFuzzer3> TypeAlias;
                AffixFuzzer4 self;
                self._tag = detail::AffixFuzzer4Tag::many_required;
                new (&self._data.many_required) TypeAlias(std::move(many_required));
                return self;
            }

            static AffixFuzzer4 many_optional(
                std::optional<std::vector<rerun::datatypes::AffixFuzzer3>> many_optional
            ) {
                typedef std::optional<std::vector<rerun::datatypes::AffixFuzzer3>> TypeAlias;
                AffixFuzzer4 self;
                self._tag = detail::AffixFuzzer4Tag::many_optional;
                new (&self._data.many_optional) TypeAlias(std::move(many_optional));
                return self;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType> &arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
                arrow::MemoryPool *memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::DenseUnionBuilder *builder, const AffixFuzzer4 *elements, size_t num_elements
            );

          private:
            detail::AffixFuzzer4Tag _tag;
            detail::AffixFuzzer4Data _data;
        };
    } // namespace datatypes
} // namespace rerun
