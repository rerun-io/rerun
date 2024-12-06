use re_log_types::LogMsg;
use re_protos::TypeConversionError;

use crate::codec::CodecError;
use crate::Compression;

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub(crate) enum MessageKind {
    SetStoreInfo = 1,
    ArrowMsg = 2,
    BlueprintActivationCommand = 3,
    End = 255,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MessageHeader {
    pub(crate) kind: MessageKind,
    pub(crate) len: u32,
}

#[cfg(feature = "encoder")]
pub(crate) mod encoder;

#[cfg(feature = "decoder")]
pub(crate) mod decoder;
