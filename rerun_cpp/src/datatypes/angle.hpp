// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/angle.fbs"

#pragma once

#include <cstdint>
#include <utility>

namespace rr {
    namespace detail {
        enum AngleTag {
            Tag_Radians,
            Tag_Degrees,
        };

        union AngleData {
            float radians;

            float degrees;

            ~AngleData() {}
        };

    } // namespace detail

    namespace datatypes {
        /// Angle in either radians or degrees.
        struct Angle {
          private:
            detail::AngleTag _tag;
            detail::AngleData _data;

          public:
            ~Angle() {
                switch (this->_tag) {
                    case detail::Tag_Radians: {
                        // TODO(#2647): code-gen for C++
                        break;
                    }
                    case detail::Tag_Degrees: {
                        // TODO(#2647): code-gen for C++
                        break;
                    }
                }
            }
        };
    } // namespace datatypes
} // namespace rr
