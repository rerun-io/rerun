use super::TensorBuffer;
use crate::tensor_data::TensorDataType;

impl TensorBuffer {
    /// The underlying data type of the buffer.
    pub fn dtype(&self) -> TensorDataType {
        match self {
            Self::U8(_) => TensorDataType::U8,
            Self::U16(_) => TensorDataType::U16,
            Self::U32(_) => TensorDataType::U32,
            Self::U64(_) => TensorDataType::U64,
            Self::I8(_) => TensorDataType::I8,
            Self::I16(_) => TensorDataType::I16,
            Self::I32(_) => TensorDataType::I32,
            Self::I64(_) => TensorDataType::I64,
            Self::F16(_) => TensorDataType::F16,
            Self::F32(_) => TensorDataType::F32,
            Self::F64(_) => TensorDataType::F64,
        }
    }

    /// The size of the buffer in bytes.
    pub fn size_in_bytes(&self) -> usize {
        match self {
            Self::U8(buf) => buf.inner().len(),
            Self::U16(buf) => buf.inner().len(),
            Self::U32(buf) => buf.inner().len(),
            Self::U64(buf) => buf.inner().len(),
            Self::I8(buf) => buf.inner().len(),
            Self::I16(buf) => buf.inner().len(),
            Self::I32(buf) => buf.inner().len(),
            Self::I64(buf) => buf.inner().len(),
            Self::F16(buf) => buf.inner().len(),
            Self::F32(buf) => buf.inner().len(),
            Self::F64(buf) => buf.inner().len(),
        }
    }

    /// Is this buffer empty?
    pub fn is_empty(&self) -> bool {
        self.size_in_bytes() == 0
    }
}

impl std::fmt::Debug for TensorBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::U8(_) => write!(f, "U8({} bytes)", self.size_in_bytes()),
            Self::U16(_) => write!(f, "U16({} bytes)", self.size_in_bytes()),
            Self::U32(_) => write!(f, "U32({} bytes)", self.size_in_bytes()),
            Self::U64(_) => write!(f, "U64({} bytes)", self.size_in_bytes()),
            Self::I8(_) => write!(f, "I8({} bytes)", self.size_in_bytes()),
            Self::I16(_) => write!(f, "I16({} bytes)", self.size_in_bytes()),
            Self::I32(_) => write!(f, "I32({} bytes)", self.size_in_bytes()),
            Self::I64(_) => write!(f, "I64({} bytes)", self.size_in_bytes()),
            Self::F16(_) => write!(f, "F16({} bytes)", self.size_in_bytes()),
            Self::F32(_) => write!(f, "F32({} bytes)", self.size_in_bytes()),
            Self::F64(_) => write!(f, "F64({} bytes)", self.size_in_bytes()),
        }
    }
}
