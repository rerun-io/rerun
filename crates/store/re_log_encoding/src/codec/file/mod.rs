#[cfg(feature = "decoder")]
pub(crate) mod decoder;
#[cfg(feature = "encoder")]
pub(crate) mod encoder;

#[allow(dead_code)] // used in encoder/decoder behind feature flag
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub(crate) enum MessageKind {
    SetStoreInfo = 1,
    ArrowMsg = 2,
    BlueprintActivationCommand = 3,
    End = 255,
}

#[allow(dead_code)] // used in encoder/decoder behind feature flag
#[derive(Debug, Clone, Copy)]
pub(crate) struct MessageHeader {
    pub(crate) kind: MessageKind,
    pub(crate) len: u32,
}
