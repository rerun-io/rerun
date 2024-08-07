use half::f16;
use ndarray::ArrayViewD;

use re_types::{
    datatypes::{ChannelDatatype, PixelFormat},
    tensor_data::TensorDataType,
};

use crate::ImageInfo;

/// Stats about a tensor or image.
#[derive(Clone, Copy, Debug)]
pub struct TensorStats {
    /// This will currently only be `None` for jpeg-encoded tensors.
    pub range: Option<(f64, f64)>,

    /// Like `range`, but ignoring all `NaN`/inf values.
    ///
    /// None if there are no finite values at all, or if the tensor is jpeg-encoded.
    pub finite_range: Option<(f64, f64)>,
}

impl TensorStats {
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
                    #[allow(trivial_numeric_casts)]
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

        #[allow(clippy::needless_pass_by_value)]
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
                    #[allow(trivial_numeric_casts)]
                    (min as f64, max as f64)
                }
            };
        }

        // declare_tensor_range_float!(tensor_range_f16, half::f16);
        declare_slice_finite_range_float!(slice_finite_range_f32, f32);
        declare_slice_finite_range_float!(slice_finite_range_f64, f64);

        #[allow(clippy::needless_pass_by_value)]
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
            Some(PixelFormat::NV12 | PixelFormat::YUY2) => {
                // We do the lazy thing here:
                return Self {
                    range: Some((0.0, 255.0)),
                    finite_range: Some((0.0, 255.0)),
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

        let finite_range = if range.0.is_finite() && range.1.is_finite() {
            // Already finite
            Some(range)
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
                Some(finite_range)
            } else {
                None
            }
        };

        Self {
            range: Some(range),
            finite_range,
        }
    }

    pub fn from_tensor(tensor: &re_types::datatypes::TensorData) -> Self {
        re_tracing::profile_function!();

        macro_rules! declare_tensor_range_int {
            ($name:ident, $typ:ty) => {
                fn $name(tensor: ndarray::ArrayViewD<'_, $typ>) -> (f64, f64) {
                    re_tracing::profile_function!();
                    let (min, max) = tensor
                        .fold((<$typ>::MAX, <$typ>::MIN), |(min, max), &value| {
                            (min.min(value), max.max(value))
                        });
                    (min as f64, max as f64)
                }
            };
        }

        macro_rules! declare_tensor_range_float {
            ($name:ident, $typ:ty) => {
                fn $name(tensor: ndarray::ArrayViewD<'_, $typ>) -> (f64, f64) {
                    re_tracing::profile_function!();
                    let (min, max) = tensor.fold(
                        (<$typ>::INFINITY, <$typ>::NEG_INFINITY),
                        |(min, max), &value| (min.min(value), max.max(value)),
                    );
                    #[allow(trivial_numeric_casts)]
                    (min as f64, max as f64)
                }
            };
        }

        declare_tensor_range_int!(tensor_range_u8, u8);
        declare_tensor_range_int!(tensor_range_u16, u16);
        declare_tensor_range_int!(tensor_range_u32, u32);
        declare_tensor_range_int!(tensor_range_u64, u64);

        declare_tensor_range_int!(tensor_range_i8, i8);
        declare_tensor_range_int!(tensor_range_i16, i16);
        declare_tensor_range_int!(tensor_range_i32, i32);
        declare_tensor_range_int!(tensor_range_i64, i64);

        // declare_tensor_range_float!(tensor_range_f16, half::f16);
        declare_tensor_range_float!(tensor_range_f32, f32);
        declare_tensor_range_float!(tensor_range_f64, f64);

        #[allow(clippy::needless_pass_by_value)]
        fn tensor_range_f16(tensor: ndarray::ArrayViewD<'_, f16>) -> (f64, f64) {
            re_tracing::profile_function!();
            let (min, max) = tensor
                .fold((f16::INFINITY, f16::NEG_INFINITY), |(min, max), &value| {
                    (min.min(value), max.max(value))
                });
            (min.to_f64(), max.to_f64())
        }

        macro_rules! declare_tensor_finite_range_float {
            ($name:ident, $typ:ty) => {
                fn $name(tensor: ndarray::ArrayViewD<'_, $typ>) -> (f64, f64) {
                    re_tracing::profile_function!();
                    let (min, max) = tensor.fold(
                        (<$typ>::INFINITY, <$typ>::NEG_INFINITY),
                        |(min, max), &value| {
                            if value.is_finite() {
                                (min.min(value), max.max(value))
                            } else {
                                (min, max)
                            }
                        },
                    );
                    #[allow(trivial_numeric_casts)]
                    (min as f64, max as f64)
                }
            };
        }

        // declare_tensor_range_float!(tensor_range_f16, half::f16);
        declare_tensor_finite_range_float!(tensor_finite_range_f32, f32);
        declare_tensor_finite_range_float!(tensor_finite_range_f64, f64);

        #[allow(clippy::needless_pass_by_value)]
        fn tensor_finite_range_f16(tensor: ndarray::ArrayViewD<'_, f16>) -> (f64, f64) {
            re_tracing::profile_function!();
            let (min, max) =
                tensor.fold((f16::INFINITY, f16::NEG_INFINITY), |(min, max), &value| {
                    if value.is_finite() {
                        (min.min(value), max.max(value))
                    } else {
                        (min, max)
                    }
                });
            (min.to_f64(), max.to_f64())
        }

        let range = match tensor.dtype() {
            TensorDataType::U8 => ArrayViewD::<u8>::try_from(tensor).map(tensor_range_u8),
            TensorDataType::U16 => ArrayViewD::<u16>::try_from(tensor).map(tensor_range_u16),
            TensorDataType::U32 => ArrayViewD::<u32>::try_from(tensor).map(tensor_range_u32),
            TensorDataType::U64 => ArrayViewD::<u64>::try_from(tensor).map(tensor_range_u64),

            TensorDataType::I8 => ArrayViewD::<i8>::try_from(tensor).map(tensor_range_i8),
            TensorDataType::I16 => ArrayViewD::<i16>::try_from(tensor).map(tensor_range_i16),
            TensorDataType::I32 => ArrayViewD::<i32>::try_from(tensor).map(tensor_range_i32),
            TensorDataType::I64 => ArrayViewD::<i64>::try_from(tensor).map(tensor_range_i64),

            TensorDataType::F16 => ArrayViewD::<f16>::try_from(tensor).map(tensor_range_f16),
            TensorDataType::F32 => ArrayViewD::<f32>::try_from(tensor).map(tensor_range_f32),
            TensorDataType::F64 => ArrayViewD::<f64>::try_from(tensor).map(tensor_range_f64),
        };

        let finite_range = if range
            .as_ref()
            .ok()
            .map_or(true, |r| r.0.is_finite() && r.1.is_finite())
        {
            range.clone().ok()
        } else {
            let finite_range = match tensor.dtype() {
                TensorDataType::U8
                | TensorDataType::U16
                | TensorDataType::U32
                | TensorDataType::U64
                | TensorDataType::I8
                | TensorDataType::I16
                | TensorDataType::I32
                | TensorDataType::I64 => range.clone(),

                TensorDataType::F16 => {
                    ArrayViewD::<f16>::try_from(tensor).map(tensor_finite_range_f16)
                }
                TensorDataType::F32 => {
                    ArrayViewD::<f32>::try_from(tensor).map(tensor_finite_range_f32)
                }
                TensorDataType::F64 => {
                    ArrayViewD::<f64>::try_from(tensor).map(tensor_finite_range_f64)
                }
            };

            // If we didn't find a finite range, set it to None.
            finite_range.ok().and_then(|r| {
                if r.0.is_finite() && r.1.is_finite() {
                    Some(r)
                } else {
                    None
                }
            })
        };

        Self {
            range: range.ok(),
            finite_range,
        }
    }
}
