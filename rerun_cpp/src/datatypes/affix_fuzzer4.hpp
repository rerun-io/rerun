// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#pragma once

#include <cstdint>
#include <cstring>
#include <new>
#include <optional>
#include <utility>
#include <vector>

#include "../datatypes/affix_fuzzer3.hpp"

namespace rr {
    namespace datatypes {
        namespace detail {
            enum class AffixFuzzer4Tag {
                /// Having a special empty state makes it possible to implement move-semantics. We
                /// need to be able to leave the object in a state which we can run the destructor
                /// on.
                NONE = 0,
                single_required,
                many_required,
                many_optional,
            };

            union AffixFuzzer4Data {
                rr::datatypes::AffixFuzzer3 single_required;

                std::vector<rr::datatypes::AffixFuzzer3> many_required;

                std::optional<std::vector<rr::datatypes::AffixFuzzer3>> many_optional;

                AffixFuzzer4Data() {}

                ~AffixFuzzer4Data() {}

                void swap(AffixFuzzer4Data& other) noexcept {
                    // This bitwise swap would fail for self-referential types, but we don't have
                    // any of those.
                    char temp[sizeof(AffixFuzzer4Data)];
                    std::memcpy(temp, this, sizeof(AffixFuzzer4Data));
                    std::memcpy(this, &other, sizeof(AffixFuzzer4Data));
                    std::memcpy(&other, temp, sizeof(AffixFuzzer4Data));
                }
            };
        } // namespace detail

        struct AffixFuzzer4 {
          private:
            detail::AffixFuzzer4Tag _tag;
            detail::AffixFuzzer4Data _data;

            AffixFuzzer4() : _tag(detail::AffixFuzzer4Tag::NONE) {}

          public:
            AffixFuzzer4(AffixFuzzer4&& other) noexcept : _tag(detail::AffixFuzzer4Tag::NONE) {
                this->swap(other);
            }

            AffixFuzzer4& operator=(AffixFuzzer4&& other) noexcept {
                this->swap(other);
                return *this;
            }

            ~AffixFuzzer4() {
                switch (this->_tag) {
                    case detail::AffixFuzzer4Tag::NONE: {
                        break; // Nothing to destroy
                    }
                    case detail::AffixFuzzer4Tag::single_required: {
                        typedef rr::datatypes::AffixFuzzer3 TypeAlias;
                        _data.single_required.~TypeAlias();
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_required: {
                        typedef std::vector<rr::datatypes::AffixFuzzer3> TypeAlias;
                        _data.many_required.~TypeAlias();
                        break;
                    }
                    case detail::AffixFuzzer4Tag::many_optional: {
                        typedef std::optional<std::vector<rr::datatypes::AffixFuzzer3>> TypeAlias;
                        _data.many_optional.~TypeAlias();
                        break;
                    }
                }
            }

            static AffixFuzzer4 single_required(rr::datatypes::AffixFuzzer3 single_required) {
                typedef rr::datatypes::AffixFuzzer3 TypeAlias;
                AffixFuzzer4 self;
                self._tag = detail::AffixFuzzer4Tag::single_required;
                new (&self._data.single_required) TypeAlias(std::move(single_required));
                return std::move(self);
            }

            static AffixFuzzer4 many_required(
                std::vector<rr::datatypes::AffixFuzzer3> many_required) {
                typedef std::vector<rr::datatypes::AffixFuzzer3> TypeAlias;
                AffixFuzzer4 self;
                self._tag = detail::AffixFuzzer4Tag::many_required;
                new (&self._data.many_required) TypeAlias(std::move(many_required));
                return std::move(self);
            }

            static AffixFuzzer4 many_optional(
                std::optional<std::vector<rr::datatypes::AffixFuzzer3>> many_optional) {
                typedef std::optional<std::vector<rr::datatypes::AffixFuzzer3>> TypeAlias;
                AffixFuzzer4 self;
                self._tag = detail::AffixFuzzer4Tag::many_optional;
                new (&self._data.many_optional) TypeAlias(std::move(many_optional));
                return std::move(self);
            }

            void swap(AffixFuzzer4& other) noexcept {
                auto tag_temp = this->_tag;
                this->_tag = other._tag;
                other._tag = tag_temp;
                this->_data.swap(other._data);
            }
        };
    } // namespace datatypes
} // namespace rr
