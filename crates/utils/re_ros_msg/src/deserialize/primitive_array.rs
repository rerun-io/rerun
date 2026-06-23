use std::fmt;

#[derive(Clone, PartialEq)]
pub enum PrimitiveArray {
    Bool(Vec<bool>),
    I8(Vec<i8>),
    U8(Vec<u8>),
    I16(Vec<i16>),
    U16(Vec<u16>),
    I32(Vec<i32>),
    U32(Vec<u32>),
    I64(Vec<i64>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    String(Vec<String>),
}

impl std::fmt::Debug for PrimitiveArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "BoolArray({})", v.len()),
            Self::I8(v) => write!(f, "I8Array({})", v.len()),
            Self::U8(v) => write!(f, "U8Array({})", v.len()),
            Self::I16(v) => write!(f, "I16Array({})", v.len()),
            Self::U16(v) => write!(f, "U16Array({})", v.len()),
            Self::I32(v) => write!(f, "I32Array({})", v.len()),
            Self::U32(v) => write!(f, "U32Array({})", v.len()),
            Self::I64(v) => write!(f, "I64Array({})", v.len()),
            Self::U64(v) => write!(f, "U64Array({})", v.len()),
            Self::F32(v) => write!(f, "F32Array({})", v.len()),
            Self::F64(v) => write!(f, "F64Array({})", v.len()),
            Self::String(v) => write!(f, "StringArray({})", v.len()),
        }
    }
}
