// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/rotation3d.fbs"

#pragma once

#include "../datatypes/quaternion.hpp"
#include "../datatypes/rotation_axis_angle.hpp"

#include <cstdint>
#include <cstring>
#include <memory>
#include <utility>

namespace rr {
    namespace datatypes {
        namespace detail {
            enum class Rotation3DTag {
                /// Having a special empty state makes it possible to implement move-semantics. We
                /// need to be able to leave the object in a state which we can run the destructor
                /// on.
                NONE = 0,
                Quaternion,
                AxisAngle,
            };

            union Rotation3DData {
                /// Rotation defined by a quaternion.
                rr::datatypes::Quaternion quaternion;

                /// Rotation defined with an axis and an angle.
                rr::datatypes::RotationAxisAngle axis_angle;

                Rotation3DData() {}

                ~Rotation3DData() {}

                void swap(Rotation3DData& other) noexcept {
                    // This bitwise swap would fail for self-referential types, but we don't have
                    // any of those.
                    char temp[sizeof(Rotation3DData)];
                    std::memcpy(temp, this, sizeof(Rotation3DData));
                    std::memcpy(this, &other, sizeof(Rotation3DData));
                    std::memcpy(&other, temp, sizeof(Rotation3DData));
                }
            };
        } // namespace detail

        /// A 3D rotation.
        struct Rotation3D {
          private:
            detail::Rotation3DTag _tag;
            detail::Rotation3DData _data;

            Rotation3D() : _tag(detail::Rotation3DTag::NONE) {}

          public:
            Rotation3D(Rotation3D&& other) noexcept : _tag(detail::Rotation3DTag::NONE) {
                this->swap(other);
            }

            Rotation3D& operator=(Rotation3D&& other) noexcept {
                this->swap(other);
                return *this;
            }

            /// Rotation defined by a quaternion.
            static Rotation3D quaternion(rr::datatypes::Quaternion quaternion) {
                Rotation3D self;
                self._tag = detail::Rotation3DTag::Quaternion;
                self._data.quaternion = std::move(quaternion);
                return std::move(self);
            }

            /// Rotation defined with an axis and an angle.
            static Rotation3D axis_angle(rr::datatypes::RotationAxisAngle axis_angle) {
                Rotation3D self;
                self._tag = detail::Rotation3DTag::AxisAngle;
                self._data.axis_angle = std::move(axis_angle);
                return std::move(self);
            }

            /// Rotation defined by a quaternion.
            Rotation3D(rr::datatypes::Quaternion quaternion) {
                *this = Rotation3D::quaternion(std::move(quaternion));
            }

            /// Rotation defined with an axis and an angle.
            Rotation3D(rr::datatypes::RotationAxisAngle axis_angle) {
                *this = Rotation3D::axis_angle(std::move(axis_angle));
            }

            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();

            void swap(Rotation3D& other) noexcept {
                auto tag_temp = this->_tag;
                this->_tag = other._tag;
                other._tag = tag_temp;
                this->_data.swap(other._data);
            }
        };
    } // namespace datatypes
} // namespace rr
