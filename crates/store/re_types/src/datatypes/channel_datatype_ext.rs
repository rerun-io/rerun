use half::f16;

use super::ChannelDatatype;

impl ChannelDatatype {
    /// Number of bits used to represent this element type.
    #[inline]
    pub fn bits(self) -> usize {
        #[expect(clippy::match_same_arms)]
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::U32 => 32,
            Self::U64 => 64,

            Self::I8 => 8,
            Self::I16 => 16,
            Self::I32 => 32,
            Self::I64 => 64,

            Self::F16 => 16,
            Self::F32 => 32,
            Self::F64 => 64,
        }
    }

    /// Is this datatype an integer?
    #[inline]
    pub fn is_integer(&self) -> bool {
        !self.is_float()
    }

    /// Is this datatype a floating point number?
    #[inline]
    pub fn is_float(&self) -> bool {
        match self {
            Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64 => false,
            Self::F16 | Self::F32 | Self::F64 => true,
        }
    }

    /// What is the minimum finite value representable by this datatype?
    #[inline]
    pub fn min_value(&self) -> f64 {
        match self {
            Self::U8 => u8::MIN as _,
            Self::U16 => u16::MIN as _,
            Self::U32 => u32::MIN as _,
            Self::U64 => u64::MIN as _,

            Self::I8 => i8::MIN as _,
            Self::I16 => i16::MIN as _,
            Self::I32 => i32::MIN as _,
            Self::I64 => i64::MIN as _,

            Self::F16 => f16::MIN.into(),
            Self::F32 => f32::MIN as _,
            Self::F64 => f64::MIN,
        }
    }

    /// What is the maximum finite value representable by this datatype?
    #[inline]
    pub fn max_value(&self) -> f64 {
        match self {
            Self::U8 => u8::MAX as _,
            Self::U16 => u16::MAX as _,
            Self::U32 => u32::MAX as _,
            Self::U64 => u64::MAX as _,

            Self::I8 => i8::MAX as _,
            Self::I16 => i16::MAX as _,
            Self::I32 => i32::MAX as _,
            Self::I64 => i64::MAX as _,

            Self::F16 => f16::MAX.into(),
            Self::F32 => f32::MAX as _,
            Self::F64 => f64::MAX,
        }
    }
}
