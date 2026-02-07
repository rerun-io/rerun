use half::f16;
use ndarray::ArrayViewD;
use re_sdk_types::tensor_data::TensorDataType;

/// Stats about a tensor or image.
#[derive(Clone, Copy, Debug)]
pub struct TensorStats {
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

impl re_byte_size::SizeBytes for TensorStats {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    fn is_pod() -> bool {
        true
    }
}

impl TensorStats {
    pub fn from_tensor(tensor: &re_sdk_types::datatypes::TensorData) -> Self {
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
                    #[allow(clippy::allow_attributes, trivial_numeric_casts)]
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

        #[expect(clippy::needless_pass_by_value)]
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
                    #[allow(clippy::allow_attributes, trivial_numeric_casts)]
                    (min as f64, max as f64)
                }
            };
        }

        // declare_tensor_range_float!(tensor_range_f16, half::f16);
        declare_tensor_finite_range_float!(tensor_finite_range_f32, f32);
        declare_tensor_finite_range_float!(tensor_finite_range_f64, f64);

        #[expect(clippy::needless_pass_by_value)]
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
        }
        .ok();

        if let Some((min, max)) = range
            && max < min
        {
            // Empty tensor
            return Self {
                range: None,
                finite_range: (tensor.dtype().min_value(), tensor.dtype().max_value()),
            };
        }

        let finite_range = if range
            .as_ref()
            .is_none_or(|r| r.0.is_finite() && r.1.is_finite())
        {
            range
        } else {
            let finite_range = match tensor.dtype() {
                TensorDataType::U8
                | TensorDataType::U16
                | TensorDataType::U32
                | TensorDataType::U64
                | TensorDataType::I8
                | TensorDataType::I16
                | TensorDataType::I32
                | TensorDataType::I64 => range,

                TensorDataType::F16 => ArrayViewD::<f16>::try_from(tensor)
                    .ok()
                    .map(tensor_finite_range_f16),
                TensorDataType::F32 => ArrayViewD::<f32>::try_from(tensor)
                    .ok()
                    .map(tensor_finite_range_f32),
                TensorDataType::F64 => ArrayViewD::<f64>::try_from(tensor)
                    .ok()
                    .map(tensor_finite_range_f64),
            };

            // If we didn't find a finite range, set it to None.
            finite_range.and_then(|r| {
                if r.0.is_finite() && r.1.is_finite() {
                    Some(r)
                } else {
                    None
                }
            })
        }
        .unwrap_or_else(|| (tensor.dtype().min_value(), tensor.dtype().max_value()));

        Self {
            range,
            finite_range,
        }
    }
}
