use std::{fs::File, io::BufWriter};

use crossbeam::channel::Receiver;
use re_log_encoding::encoder::DroppableEncoder;
use re_protos::{
    common::v1alpha1::{ApplicationId, StoreId, StoreKind},
    log_msg::v1alpha1::{ArrowMsg, LogMsg, SetStoreInfo, StoreInfo, log_msg::Msg},
};

use crate::commands::{read_raw_rrd_streams_from_file_or_stdin, stdio::InputSource};

#[derive(Debug, Clone, clap::Parser)]
pub struct RouteCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    ///
    /// Blueprints are currently dropped from the input.
    path_to_input_rrds: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.rrd")]
    path_to_output_rrd: Option<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,

    /// If set, specifies the application id of the resulting recordings.
    #[clap(long = "application-id")]
    application_id: Option<String>,

    /// If set, specifies the recording id of the resulting recordings.
    #[clap(long = "recording-id")]
    recording_id: Option<String>,
}

struct Rewrites {
    application_id: Option<ApplicationId>,
    store_id: Option<StoreId>,
}

impl RouteCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            continue_on_error,
            application_id,
            recording_id,
        } = self;

        let rewrites = Rewrites {
            application_id: application_id
                .as_ref()
                .map(|id| ApplicationId { id: id.clone() }),
            store_id: recording_id.as_ref().map(|id| StoreId {
                id: id.clone(),
                kind: StoreKind::Recording.into(),
            }),
        };

        let (rx, _) = read_raw_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        if let Some(path) = path_to_output_rrd {
            let writer = BufWriter::new(File::create(path)?);
            process_messages(&rewrites, *continue_on_error, writer, &rx)?;
        } else {
            let stdout = std::io::stdout();
            let lock = stdout.lock();
            let writer = BufWriter::new(lock);
            process_messages(&rewrites, *continue_on_error, writer, &rx)?;
        }

        Ok(())
    }
}

fn process_messages<W: std::io::Write>(
    rewrites: &Rewrites,
    continue_on_error: bool,
    writer: W,
    receiver: &Receiver<(InputSource, anyhow::Result<Msg>)>,
) -> anyhow::Result<()> {
    re_log::info!("processing input…");
    let mut num_total_msgs = 0;
    let mut num_unexpected_msgs = 0;
    let mut num_blueprints_msgs = 0;

    // TODO(grtlr): encoding should match the original
    let options = re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = re_build_info::CrateVersion::LOCAL;
    let mut encoder = DroppableEncoder::new(version, options, writer)?;

    while let Ok((_input, res)) = receiver.recv() {
        let mut is_success = true;

        match res {
            Ok(mut msg) => {
                num_total_msgs += 1;

                if is_blueprint(&msg) {
                    num_blueprints_msgs += 1;
                    continue;
                }

                match &mut msg {
                    Msg::SetStoreInfo(SetStoreInfo {
                        info:
                            Some(StoreInfo {
                                application_id,
                                store_id,
                                ..
                            }),
                        ..
                    }) => {
                        apply_store_id_rewrite(store_id, &rewrites.store_id);
                        apply_application_id_rewrite(application_id, &rewrites.application_id);
                    }

                    Msg::ArrowMsg(ArrowMsg { store_id, .. }) => {
                        apply_store_id_rewrite(store_id, &rewrites.store_id);
                    }

                    _ => {
                        num_unexpected_msgs += 1;
                        re_log::warn_once!("Encountered unexpected message: {:#?}", msg);
                    }
                }

                // modify msg
                let log_msg = LogMsg { msg: Some(msg) };
                encoder.append_proto(log_msg)?;
            }
            Err(err) => {
                re_log::error_once!("{}", re_error::format(err));
                is_success = false;
            }
        }

        if !continue_on_error && !is_success {
            anyhow::bail!(
                "one or more IO and/or decoding failures in the input stream (check logs)"
            )
        }
    }

    encoder.finish()?;

    re_log::info_once!(
        "Processed {num_total_msgs} messages, dropped {num_blueprints_msgs} blueprint messages, and encountered {num_unexpected_msgs} unexpected messages."
    );
    Ok(())
}

fn is_blueprint(msg: &Msg) -> bool {
    match msg {
        Msg::SetStoreInfo(SetStoreInfo {
            info:
                Some(StoreInfo {
                    store_id: Some(StoreId { kind, .. }),
                    ..
                }),
            ..
        })
        | Msg::ArrowMsg(ArrowMsg {
            store_id: Some(StoreId { kind, .. }),
            ..
        }) if *kind == StoreKind::Blueprint as i32 => true,
        Msg::BlueprintActivationCommand(_) => true,
        _ => false,
    }
}

fn apply_store_id_rewrite(store_id: &mut Option<StoreId>, target: &Option<StoreId>) {
    if let (Some(store_id), Some(target)) = (store_id, target) {
        *store_id = target.clone();
    }
}

fn apply_application_id_rewrite(
    app_id: &mut Option<ApplicationId>,
    target: &Option<ApplicationId>,
) {
    if let (Some(app_id), Some(target)) = (app_id, target) {
        *app_id = target.clone();
    }
}
