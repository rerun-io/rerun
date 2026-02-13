//! Everything needed to convert back and forth between transport-level and application-level types.
//!
//! ⚠️Make sure to familiarize yourself with the [crate-level docs] first. ⚠️
//!
//! This is where all the complex application-level logic that precedes encoding / follows decoding
//! happens: Chunk/Sorbet migrations, data patching (app ID injection, version propagation, BW-compat hacks,
//! etc).
//!
//! To go from a freshly decoded transport-level type to its application-level equivalent, use [`ToApplication`].
//! To prepare an application-level type for encoding, use [`ToTransport`].

use re_build_info::CrateVersion;
use re_log_types::{BlueprintActivationCommand, SetStoreInfo};

use crate::ApplicationIdInjector;
use crate::rrd::CodecError;

// TODO(cmc): I'd really like a nice centralized way of communicating this.
//
// pub type LogMsgTransport = re_protos::log_msg::v1alpha1::log_msg::Msg;
// pub type LogMsgApp = re_log_types::LogMsg;

// ---

/// Converts an application-level type to a transport-level type, ready for encoding.
pub trait ToTransport {
    type Output;
    type Context<'a>;

    fn to_transport(&self, context: Self::Context<'_>) -> Result<Self::Output, CodecError>;
}

impl ToTransport for re_log_types::LogMsg {
    type Output = re_protos::log_msg::v1alpha1::log_msg::Msg;
    type Context<'a> = crate::rrd::Compression;

    fn to_transport(&self, compression: Self::Context<'_>) -> Result<Self::Output, CodecError> {
        log_msg_app_to_transport(self, compression)
    }
}

impl ToTransport for re_log_types::ArrowMsg {
    type Output = re_protos::log_msg::v1alpha1::ArrowMsg;
    type Context<'a> = (re_log_types::StoreId, crate::rrd::Compression);

    fn to_transport(
        &self,
        (store_id, compression): Self::Context<'_>,
    ) -> Result<Self::Output, CodecError> {
        arrow_msg_app_to_transport(self, store_id, compression)
    }
}

impl ToTransport for crate::RrdFooter {
    type Output = re_protos::log_msg::v1alpha1::RrdFooter;
    type Context<'a> = ();

    fn to_transport(&self, _: Self::Context<'_>) -> Result<Self::Output, CodecError> {
        let manifests: Result<Vec<_>, _> = self
            .manifests
            .values()
            .map(|manifest| manifest.to_transport(()))
            .collect();

        Ok(Self::Output {
            manifests: manifests?,
        })
    }
}

impl ToTransport for crate::RawRrdManifest {
    type Output = re_protos::log_msg::v1alpha1::RrdManifest;
    type Context<'a> = ();

    fn to_transport(&self, (): Self::Context<'_>) -> Result<Self::Output, CodecError> {
        {
            self.sanity_check_cheap()?;

            // that will only work for tests local to this crate, but that's better than nothing.
            #[cfg(test)]
            self.sanity_check_heavy()?;
        }

        let sorbet_schema = re_protos::common::v1alpha1::Schema::try_from(&self.sorbet_schema)
            .map_err(CodecError::ArrowSerialization)?;

        Ok(Self::Output {
            store_id: Some(self.store_id.clone().into()),
            sorbet_schema_sha256: Some(self.sorbet_schema_sha256.to_vec().into()),
            sorbet_schema: Some(sorbet_schema),
            data: Some(self.data.clone().into()),
        })
    }
}

/// Converts a transport-level type to an application-level type, ready for use in the viewer.
pub trait ToApplication {
    type Output;
    type Context<'a>;

    fn to_application(&self, context: Self::Context<'_>) -> Result<Self::Output, CodecError>;
}

impl ToApplication for re_protos::log_msg::v1alpha1::log_msg::Msg {
    type Output = re_log_types::LogMsg;
    type Context<'a> = (&'a mut dyn ApplicationIdInjector, Option<CrateVersion>);

    fn to_application(
        &self,
        (app_id_injector, patched_version): Self::Context<'_>,
    ) -> Result<Self::Output, CodecError> {
        let mut log_msg = log_msg_transport_to_app(app_id_injector, self)?;

        if let Some(patched_version) = patched_version
            && let re_log_types::LogMsg::SetStoreInfo(msg) = &mut log_msg
        {
            // In the context of a native RRD stream (files, stdio, etc), this is used to patch the
            // version advertised by the application-level object so that it matches the one advertised
            // in the stream header.
            // This in turn is what makes it possible to display the version of the RRD file in the viewer.
            msg.info.store_version = Some(patched_version);
        }

        Ok(log_msg)
    }
}

impl ToApplication for re_protos::log_msg::v1alpha1::LogMsg {
    type Output = re_log_types::LogMsg;
    type Context<'a> = (&'a mut dyn ApplicationIdInjector, Option<CrateVersion>);

    fn to_application(
        &self,
        app_id_injector: Self::Context<'_>,
    ) -> Result<Self::Output, CodecError> {
        let Some(msg) = self.msg.as_ref() else {
            return Err(re_protos::missing_field!(Self, "msg").into());
        };

        msg.to_application(app_id_injector)
    }
}

impl ToApplication for re_protos::log_msg::v1alpha1::ArrowMsg {
    type Output = re_log_types::ArrowMsg;
    type Context<'a> = ();

    fn to_application(&self, _context: Self::Context<'_>) -> Result<Self::Output, CodecError> {
        arrow_msg_transport_to_app(self)
    }
}

impl ToApplication for re_protos::log_msg::v1alpha1::RrdFooter {
    type Output = crate::RrdFooter;
    type Context<'a> = ();

    fn to_application(&self, _context: Self::Context<'_>) -> Result<Self::Output, CodecError> {
        let manifests: Result<std::collections::HashMap<_, _>, _> = self
            .manifests
            .iter()
            .map(|manifest| {
                let manifest = manifest.to_application(())?;
                Ok::<_, CodecError>((manifest.store_id.clone(), manifest))
            })
            .collect();

        Ok(Self::Output {
            manifests: manifests?,
        })
    }
}

impl ToApplication for re_protos::log_msg::v1alpha1::RrdManifest {
    type Output = crate::RawRrdManifest;
    type Context<'a> = ();

    fn to_application(&self, _context: Self::Context<'_>) -> Result<Self::Output, CodecError> {
        let store_id = self
            .store_id
            .as_ref()
            .ok_or_else(|| re_protos::missing_field!(Self, "store_id"))?;

        let sorbet_schema = self
            .sorbet_schema
            .as_ref()
            .ok_or_else(|| re_protos::missing_field!(Self, "sorbet_schema"))?;

        let sorbet_schema_sha256 = self
            .sorbet_schema_sha256
            .as_ref()
            .ok_or_else(|| re_protos::missing_field!(Self, "sorbet_schema_sha256"))?;
        let sorbet_schema_sha256: [u8; 32] = (**sorbet_schema_sha256)
            .try_into()
            .map_err(|err| re_protos::invalid_field!(Self, "sorbet_schema_sha256", err))?;

        let data = self
            .data
            .as_ref()
            .ok_or_else(|| re_protos::missing_field!(Self, "data"))?;

        let rrd_manifest = Self::Output {
            store_id: store_id.clone().try_into()?,
            sorbet_schema: sorbet_schema
                .try_into()
                .map_err(CodecError::ArrowDeserialization)?,
            sorbet_schema_sha256,
            data: data.try_into()?,
        };

        {
            rrd_manifest.sanity_check_cheap()?;

            // that will only work for tests local to this crate, but that's better than nothing
            #[cfg(test)]
            rrd_manifest.sanity_check_heavy()?;
        }

        Ok(rrd_manifest)
    }
}

// ---

/// Converts a transport-level `LogMsg` to its application-level counterpart.
///
/// This function attempts to migrate legacy `StoreId` with missing application id. It will return
/// [`CodecError::StoreIdMissingApplicationId`] if a message arrives before the matching
/// `SetStoreInfo` message.
///
/// The provided [`ApplicationIdInjector`] must be shared across all calls for the same stream.
#[tracing::instrument(level = "trace", skip_all)]
fn log_msg_transport_to_app<I: ApplicationIdInjector + ?Sized>(
    app_id_injector: &mut I,
    message: &re_protos::log_msg::v1alpha1::log_msg::Msg,
) -> Result<re_log_types::LogMsg, CodecError> {
    re_tracing::profile_function!();

    use re_protos::log_msg::v1alpha1::log_msg::Msg;
    use re_protos::missing_field;

    match message {
        Msg::SetStoreInfo(set_store_info) => {
            let set_store_info: SetStoreInfo = set_store_info.clone().try_into()?;
            app_id_injector.store_info_received(&set_store_info.info);
            Ok(re_log_types::LogMsg::SetStoreInfo(set_store_info))
        }

        Msg::ArrowMsg(arrow_msg) => {
            let encoded = arrow_msg_transport_to_app(arrow_msg)?;

            //TODO(#10730): clean that up when removing 0.24 back compat
            let store_id: re_log_types::StoreId = match arrow_msg
                .store_id
                .as_ref()
                .ok_or_else(|| missing_field!(re_protos::log_msg::v1alpha1::ArrowMsg, "store_id"))?
                .clone()
                .try_into()
            {
                Ok(store_id) => store_id,
                Err(err) => {
                    let Some(store_id) = app_id_injector.recover_store_id(err.clone()) else {
                        return Err(err.into());
                    };

                    store_id
                }
            };

            Ok(re_log_types::LogMsg::ArrowMsg(store_id, encoded))
        }

        Msg::BlueprintActivationCommand(blueprint_activation_command) => {
            //TODO(#10730): clean that up when removing 0.24 back compat
            let blueprint_id: re_log_types::StoreId = match blueprint_activation_command
                .blueprint_id
                .as_ref()
                .ok_or_else(|| {
                    missing_field!(
                        re_protos::log_msg::v1alpha1::BlueprintActivationCommand,
                        "blueprint_id"
                    )
                })?
                .clone()
                .try_into()
            {
                Ok(store_id) => store_id,
                Err(err) => {
                    let Some(store_id) = app_id_injector.recover_store_id(err.clone()) else {
                        return Err(err.into());
                    };

                    store_id
                }
            };

            Ok(re_log_types::LogMsg::BlueprintActivationCommand(
                BlueprintActivationCommand {
                    blueprint_id,
                    make_active: blueprint_activation_command.make_active,
                    make_default: blueprint_activation_command.make_default,
                },
            ))
        }
    }
}

/// Converts a transport-level `ArrowMsg` to its application-level counterpart.
#[tracing::instrument(level = "trace", skip_all)]
fn arrow_msg_transport_to_app(
    arrow_msg: &re_protos::log_msg::v1alpha1::ArrowMsg,
) -> Result<re_log_types::ArrowMsg, CodecError> {
    re_tracing::profile_function!();

    use re_protos::log_msg::v1alpha1::Encoding;

    if arrow_msg.encoding() != Encoding::ArrowIpc {
        return Err(CodecError::UnsupportedEncoding);
    }

    let batch = decode_arrow(
        &arrow_msg.payload,
        arrow_msg.uncompressed_size as usize,
        arrow_msg.compression().into(),
    )?;

    let chunk_id = re_sorbet::chunk_id_of_schema(batch.schema_ref())?.as_tuid();

    // TODO(grtlr): In the future, we should be able to rely on the `chunk_id` to be present in the
    // protobuf definitions. For now we have to extract it from the `batch`.
    //
    // let chunk_id = arrow_msg
    //     .chunk_id
    //     .ok_or_else(|| missing_field!(re_protos::log_msg::v1alpha1::ArrowMsg, "chunk_id"))?
    //     .try_from()?;

    // This also ensures that we perform all required migrations from `re_sorbet`.
    // TODO(#10343): Would it make sense to change `re_types_core::ArrowMsg` to contain the
    // `ChunkBatch` directly?
    let chunk_batch = re_sorbet::ChunkBatch::try_from(&batch)?;

    // TODO(emilk): it would actually be nicer if we could postpone the migration,
    // so that there is some way to get the original (unmigrated) data out of an .rrd,
    // which would be very useful for debugging, e.g. using the `print` command.

    Ok(re_log_types::ArrowMsg {
        chunk_id,
        batch: chunk_batch.into(),
        on_release: None,
    })
}

/// Converts an application-level `LogMsg` to its transport-level counterpart.
#[tracing::instrument(level = "trace", skip_all)]
fn log_msg_app_to_transport(
    message: &re_log_types::LogMsg,
    compression: crate::rrd::Compression,
) -> Result<re_protos::log_msg::v1alpha1::log_msg::Msg, CodecError> {
    re_tracing::profile_function!();

    let proto_msg = match message {
        re_log_types::LogMsg::SetStoreInfo(set_store_info) => {
            re_protos::log_msg::v1alpha1::log_msg::Msg::SetStoreInfo(set_store_info.clone().into())
        }

        re_log_types::LogMsg::ArrowMsg(store_id, arrow_msg) => {
            let arrow_msg = arrow_msg_app_to_transport(arrow_msg, store_id.clone(), compression)?;
            re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(arrow_msg)
        }

        re_log_types::LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
            re_protos::log_msg::v1alpha1::log_msg::Msg::BlueprintActivationCommand(
                blueprint_activation_command.clone().into(),
            )
        }
    };

    Ok(proto_msg)
}

/// Converts an application-level `ArrowMsg` to its transport-level counterpart.
#[tracing::instrument(level = "trace", skip_all)]
fn arrow_msg_app_to_transport(
    arrow_msg: &re_log_types::ArrowMsg,
    store_id: re_log_types::StoreId,
    compression: crate::rrd::Compression,
) -> Result<re_protos::log_msg::v1alpha1::ArrowMsg, CodecError> {
    re_tracing::profile_function!();

    let re_log_types::ArrowMsg {
        chunk_id,
        batch,
        on_release: _,
    } = arrow_msg;

    let payload = encode_arrow(batch, compression)?;

    Ok(re_protos::log_msg::v1alpha1::ArrowMsg {
        store_id: Some(store_id.into()),
        chunk_id: Some((*chunk_id).into()),
        compression: re_protos::common::v1alpha1::Compression::from(compression) as i32,
        uncompressed_size: payload.uncompressed_size,
        encoding: re_protos::log_msg::v1alpha1::Encoding::ArrowIpc as i32,
        payload: payload.data.into(),
        is_static: re_sorbet::is_static_chunk(batch),
    })
}

// ---

struct EncodedArrowRecordBatch {
    uncompressed_size: u64,
    data: Vec<u8>,
}

/// Encodes a native `RecordBatch` to an IPC payload, optionally compressed.
#[tracing::instrument(level = "debug", skip_all)]
fn encode_arrow(
    batch: &arrow::array::RecordBatch,
    compression: crate::rrd::Compression,
) -> Result<EncodedArrowRecordBatch, CodecError> {
    re_tracing::profile_function!();

    let mut uncompressed = Vec::new();
    {
        let schema = batch.schema_ref().as_ref();

        let mut sw = {
            let _span = tracing::trace_span!("schema").entered();
            ::arrow::ipc::writer::StreamWriter::try_new(&mut uncompressed, schema)
                .map_err(CodecError::ArrowSerialization)?
        };

        {
            let _span = tracing::trace_span!("data").entered();
            sw.write(batch).map_err(CodecError::ArrowSerialization)?;
        }

        sw.finish().map_err(CodecError::ArrowSerialization)?;
    }

    // This will never fail until we have 128bit-native CPUs, but `as` is too dangerous and very
    // sensitive to refactorings.
    let uncompressed_size = uncompressed.len().try_into()?;

    let data = match compression {
        crate::rrd::Compression::Off => uncompressed,
        crate::rrd::Compression::LZ4 => {
            re_tracing::profile_scope!("lz4::compress");
            let _span = tracing::trace_span!("lz4::compress").entered();
            lz4_flex::block::compress(&uncompressed)
        }
    };

    Ok(EncodedArrowRecordBatch {
        uncompressed_size,
        data,
    })
}

/// Decodes a potentially compressed IPC payload into a native `RecordBatch`.
//
// TODO(cmc): can we use the File-oriented APIs in order to re-use the transport buffer as backing
// storage for the final RecordBatch?
// See e.g. https://github.com/apache/arrow-rs/blob/b8b2f21f6a8254224d37a1e2d231b6b1e1767648/arrow/examples/zero_copy_ipc.rs
#[tracing::instrument(level = "debug", skip_all)]
fn decode_arrow(
    data: &[u8],
    uncompressed_size: usize,
    compression: crate::rrd::Compression,
) -> Result<arrow::array::RecordBatch, CodecError> {
    let mut uncompressed = Vec::new();
    let data = match compression {
        crate::rrd::Compression::Off => data,
        crate::rrd::Compression::LZ4 => {
            re_tracing::profile_scope!("LZ4-decompress");
            let _span = tracing::trace_span!("lz4::decompress").entered();
            uncompressed.resize(uncompressed_size, 0);
            lz4_flex::block::decompress_into(data, &mut uncompressed)?;
            uncompressed.as_slice()
        }
    };

    let mut stream = {
        let _span = tracing::trace_span!("schema").entered();
        ::arrow::ipc::reader::StreamReader::try_new(data, None)
            .map_err(CodecError::ArrowDeserialization)?
    };

    let _span = tracing::trace_span!("data").entered();
    stream
        .next()
        .ok_or(CodecError::MissingRecordBatch)?
        .map_err(CodecError::ArrowDeserialization)
}
