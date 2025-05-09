use super::{MessageHeader, MessageKind};
use crate::codec::arrow::ArrowEncodingContext;
use crate::encoder::EncodeError;
use crate::Compression;
use re_log_types::LogMsg;

pub(crate) fn encode(
    buf: &mut Vec<u8>,
    arrow_ctx: &mut ArrowEncodingContext,
    message: &LogMsg,
    compression: Compression,
) -> Result<(), EncodeError> {
    use re_protos::external::prost::Message as _;
    use re_protos::log_msg::v1alpha1::{
        self as proto, ArrowMsg, BlueprintActivationCommand, Encoding, SetStoreInfo,
    };

    match message {
        LogMsg::SetStoreInfo(set_store_info) => {
            let set_store_info: SetStoreInfo = set_store_info.clone().into();
            let header = MessageHeader {
                kind: MessageKind::SetStoreInfo,
                len: set_store_info.encoded_len() as u64,
            };
            header.encode(buf)?;
            set_store_info.encode(buf)?;
        }
        LogMsg::ArrowMsg(store_id, arrow_msg) => {
            let payload = crate::codec::arrow::encode_arrow_with_ctx(
                arrow_ctx,
                &arrow_msg.batch,
                compression,
            )?;

            // Optimization: `ArrowMsg` requires an owned `Vec` for its `payload` field, but
            // with how we're using it here, it shouldn't need to be. Ideally, it would be a
            // `Cow` so that we could pass it borrowed data, but that's not something that `prost` supports.
            //
            // This optimization removes a copy of the payload, which may be very significant.
            // For a program that does nothing but log ~300 million points to `/dev/null`,
            // running on a Ryzen 9 7950x + Linux 6.13.2 (Fedora 41):
            // * The system time is reduced by ~15% (due to fewer page faults, roughly 1/10th as many),
            // * The total runtime is reduced by ~5%.
            // * Most importantly, the maximum resident set size (physical memory usage) goes from ~7 GiB to ~1 GiB.
            //
            // The entire block is marked `unsafe`, because it contains unsafe code that depends on technically
            // safe code for its soundness. We control both the unsafe and safe parts, so we can still be sure
            // we're correct here. Unsafe operations are further marked with their own `unsafe` blocks.
            //
            // SAFETY: See safety comment on `payload` below.
            #[allow(unsafe_code, unused_unsafe)]
            unsafe {
                let arrow_msg = ArrowMsg {
                    store_id: Some(store_id.clone().into()),
                    compression: match compression {
                        Compression::Off => proto::Compression::None as i32,
                        Compression::LZ4 => proto::Compression::Lz4 as i32,
                    },
                    uncompressed_size: payload.uncompressed_size as i32,
                    encoding: Encoding::ArrowIpc as i32,

                    // SAFETY: For this to be safe, we have to ensure the resulting `vec` is not resized, as that could trigger
                    // an allocation and cause the original `Vec` held in `arrow_ctx` to then contain a dangling pointer.
                    //
                    // * The variable binding for `ArrowMsg` is immutable, so only a shared (`&self`) reference may be
                    //   produced from it and to the `Vec` being constructed here, so the payload can never be resized.
                    // * We `mem::forget` the payload once we've encoded the `ArrowMsg` to avoid its `Drop` call.
                    //   * Without this, the `Vec` would free its backing storage, which we need to avoid.
                    payload: unsafe { payload.to_fake_temp_vec() },
                };
                let header = MessageHeader {
                    kind: MessageKind::ArrowMsg,
                    len: arrow_msg.encoded_len() as u64,
                };
                header.encode(buf)?;
                arrow_msg.encode(buf)?;

                // See `SAFETY` comment on `payload` above.
                #[allow(clippy::mem_forget)]
                std::mem::forget(arrow_msg.payload);
            };
        }
        LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
            let blueprint_activation_command: BlueprintActivationCommand =
                blueprint_activation_command.clone().into();
            let header = MessageHeader {
                kind: MessageKind::BlueprintActivationCommand,
                len: blueprint_activation_command.encoded_len() as u64,
            };
            header.encode(buf)?;
            blueprint_activation_command.encode(buf)?;
        }
    }

    Ok(())
}
