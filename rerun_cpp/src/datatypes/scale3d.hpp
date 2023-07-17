// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/scale3d.fbs"

#pragma once

#include <cstdint>
#include <cstring>
#include <new>
#include <utility>

#include "../datatypes/vec3d.hpp"

namespace rr {
    namespace datatypes {
        namespace detail {
            enum class Scale3DTag {
                NONE = 0, // Makes it possible to implement move semantics
                ThreeD,
                Uniform,
            };

            union Scale3DData {
                /// Individual scaling factors for each axis, distorting the original object.
                rr::datatypes::Vec3D three_d;

                /// Uniform scaling factor along all axis.
                float uniform;

                Scale3DData() {}

                ~Scale3DData() {}

                void swap(Scale3DData& other) noexcept {
                    char temp[sizeof(Scale3DData)];
                    std::memcpy(temp, this, sizeof(Scale3DData));
                    std::memcpy(this, &other, sizeof(Scale3DData));
                    std::memcpy(&other, temp, sizeof(Scale3DData));
                }
            };
        } // namespace detail

        /// 3D scaling factor, part of a transform representation.
        struct Scale3D {
          private:
            detail::Scale3DTag _tag;
            detail::Scale3DData _data;

            Scale3D() : _tag(detail::Scale3DTag::NONE) {}

          public:
            Scale3D(Scale3D&& other) noexcept : _tag(detail::Scale3DTag::NONE) {
                this->swap(other);
            }

            Scale3D& operator=(Scale3D&& other) noexcept {
                this->swap(other);
                return *this;
            }

            /// Individual scaling factors for each axis, distorting the original object.
            static Scale3D three_d(rr::datatypes::Vec3D three_d) {
                Scale3D self;
                self._tag = detail::Scale3DTag::ThreeD;
                self._data.three_d = std::move(three_d);
                return std::move(self);
            }

            /// Uniform scaling factor along all axis.
            static Scale3D uniform(float uniform) {
                Scale3D self;
                self._tag = detail::Scale3DTag::Uniform;
                self._data.uniform = std::move(uniform);
                return std::move(self);
            }

            void swap(Scale3D& other) noexcept {
                auto tag_temp = this->_tag;
                this->_tag = other._tag;
                other._tag = tag_temp;
                this->_data.swap(other._data);
            }
        };
    } // namespace datatypes
} // namespace rr
