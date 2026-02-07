use half::f16;
use re_sdk_types::datatypes::ChannelDatatype;

use crate::ImageInfo;

/// Stats about an image.
#[derive(Clone, Copy, Debug)]
pub struct ImageStats {
    /// The range of values, ignoring `NaN`s.
    ///
    /// `None` for empty tensors.
    pub range: Option<(f64, f64)>,

    /// Like `range`, but ignoring all `NaN`/inf values.
    ///
    /// If no finite values are present, this takes the maximum finite range
    /// of the underlying data type.
    pub finite_range: (f64, f64),
}

impl re_byte_size::SizeBytes for ImageStats {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    fn is_pod() -> bool {
        true
    }
}

impl ImageStats {
    pub fn from_image(image: &ImageInfo) -> Self {
        re_tracing::profile_function!();

        // TODO(#6008): support stride

        macro_rules! declare_slice_range_int {
            ($name:ident, $typ:ty) => {
                fn $name(slice: &[$typ]) -> (f64, f64) {
                    re_tracing::profile_function!();
                    let (min, max) = slice
                        .iter()
                        .fold((<$typ>::MAX, <$typ>::MIN), |(min, max), &value| {
                            (min.min(value), max.max(value))
                        });
                    (min as f64, max as f64)
                }
            };
        }

        macro_rules! declare_slice_range_float {
            ($name:ident, $typ:ty) => {
                fn $name(slice: &[$typ]) -> (f64, f64) {
                    re_tracing::profile_function!();
                    let (min, max) = slice.iter().fold(
                        (<$typ>::INFINITY, <$typ>::NEG_INFINITY),
                        |(min, max), &value| (min.min(value), max.max(value)),
                    );
                    #[allow(clippy::allow_attributes, trivial_numeric_casts)]
                    (min as f64, max as f64)
                }
            };
        }

        declare_slice_range_int!(slice_range_u8, u8);
        declare_slice_range_int!(slice_range_u16, u16);
        declare_slice_range_int!(slice_range_u32, u32);
        declare_slice_range_int!(slice_range_u64, u64);

        declare_slice_range_int!(slice_range_i8, i8);
        declare_slice_range_int!(slice_range_i16, i16);
        declare_slice_range_int!(slice_range_i32, i32);
        declare_slice_range_int!(slice_range_i64, i64);

        // declare_slice_range_float!(slice_range_f16, f16);
        declare_slice_range_float!(slice_range_f32, f32);
        declare_slice_range_float!(slice_range_f64, f64);

        fn slice_range_f16(slice: &[f16]) -> (f64, f64) {
            re_tracing::profile_function!();
            let (min, max) = slice
                .iter()
                .fold((f16::INFINITY, f16::NEG_INFINITY), |(min, max), &value| {
                    (min.min(value), max.max(value))
                });
            (min.to_f64(), max.to_f64())
        }

        macro_rules! declare_slice_finite_range_float {
            ($name:ident, $typ:ty) => {
                fn $name(slice: &[$typ]) -> (f64, f64) {
                    re_tracing::profile_function!();
                    let (min, max) = slice.iter().fold(
                        (<$typ>::INFINITY, <$typ>::NEG_INFINITY),
                        |(min, max), &value| {
                            if value.is_finite() {
                                (min.min(value), max.max(value))
                            } else {
                                (min, max)
                            }
                        },
                    );
                    #[allow(clippy::allow_attributes, trivial_numeric_casts)]
                    (min as f64, max as f64)
                }
            };
        }

        // declare_tensor_range_float!(tensor_range_f16, half::f16);
        declare_slice_finite_range_float!(slice_finite_range_f32, f32);
        declare_slice_finite_range_float!(slice_finite_range_f64, f64);

        fn slice_finite_range_f16(slice: &[f16]) -> (f64, f64) {
            re_tracing::profile_function!();
            let (min, max) =
                slice
                    .iter()
                    .fold((f16::INFINITY, f16::NEG_INFINITY), |(min, max), &value| {
                        if value.is_finite() {
                            (min.min(value), max.max(value))
                        } else {
                            (min, max)
                        }
                    });
            (min.to_f64(), max.to_f64())
        }

        // ---------------------------

        let datatype = match image.format.pixel_format {
            Some(_) => {
                // We do the lazy thing here since we convert everything to RGB8 right now anyways.
                // Note that this range is all about the format we're converting _to_.
                // It would be nice if we can distininguish this better in the future:
                // E.g. limited range YUV should have the correct limited range.
                return Self {
                    range: Some((0.0, 255.0)),
                    finite_range: (0.0, 255.0),
                };
            }
            None => image.format.datatype(),
        };

        let range = match datatype {
            ChannelDatatype::U8 => slice_range_u8(&image.to_slice()),
            ChannelDatatype::U16 => slice_range_u16(&image.to_slice()),
            ChannelDatatype::U32 => slice_range_u32(&image.to_slice()),
            ChannelDatatype::U64 => slice_range_u64(&image.to_slice()),

            ChannelDatatype::I8 => slice_range_i8(&image.to_slice()),
            ChannelDatatype::I16 => slice_range_i16(&image.to_slice()),
            ChannelDatatype::I32 => slice_range_i32(&image.to_slice()),
            ChannelDatatype::I64 => slice_range_i64(&image.to_slice()),

            ChannelDatatype::F16 => slice_range_f16(&image.to_slice()),
            ChannelDatatype::F32 => slice_range_f32(&image.to_slice()),
            ChannelDatatype::F64 => slice_range_f64(&image.to_slice()),
        };

        if range.1 < range.0 {
            // Empty image
            return Self {
                range: None,
                finite_range: (datatype.min_value(), datatype.max_value()),
            };
        }

        let finite_range = if range.0.is_finite() && range.1.is_finite() {
            // Already finite
            range
        } else {
            let finite_range = match datatype {
                ChannelDatatype::U8
                | ChannelDatatype::U16
                | ChannelDatatype::U32
                | ChannelDatatype::U64
                | ChannelDatatype::I8
                | ChannelDatatype::I16
                | ChannelDatatype::I32
                | ChannelDatatype::I64 => range,

                ChannelDatatype::F16 => slice_finite_range_f16(&image.to_slice()),
                ChannelDatatype::F32 => slice_finite_range_f32(&image.to_slice()),
                ChannelDatatype::F64 => slice_finite_range_f64(&image.to_slice()),
            };

            // Ensure it actually is finite:
            if finite_range.0.is_finite() && finite_range.1.is_finite() {
                finite_range
            } else {
                (datatype.min_value(), datatype.max_value())
            }
        };

        Self {
            range: Some(range),
            finite_range,
        }
    }
}
