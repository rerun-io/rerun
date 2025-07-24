use std::{fs::File, io::BufWriter};

use crossbeam::channel::Receiver;
use re_log_encoding::encoder::DroppableEncoder;
use re_protos::{
    common::v1alpha1::ApplicationId,
    log_msg::v1alpha1::{ArrowMsg, LogMsg, SetStoreInfo, StoreInfo, log_msg::Msg},
};

use crate::commands::{read_raw_rrd_streams_from_file_or_stdin, stdio::InputSource};

#[derive(Debug, Clone, clap::Parser)]
pub struct RouteCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.rrd")]
    path_to_output_rrd: Option<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,

    /// If set, specifies the application id of the output.
    #[clap(long = "application-id")]
    application_id: Option<String>,

    /// If set, specifies the recording id of the output.
    ///
    /// When this flag is set and multiple input .rdd files are specified,
    /// blueprint activation commands will be dropped from the resulting
    /// output.
    #[clap(long = "recording-id")]
    recording_id: Option<String>,
}

struct Rewrites {
    application_id: Option<ApplicationId>,
    recording_id: Option<String>,
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
            recording_id: recording_id.clone(),
        };

        let (rx, _) = read_raw_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        // When we merge multiple recordings with blueprints, it does not make sense to activate any of them,
        // and instead we want viewer heuristics to take over. Therefore, we drop blueprint activation
        // commands when overwriting the recording id.
        let drop_blueprint_activation_cmds =
            path_to_input_rrds.len() > 1 && rewrites.recording_id.is_some();

        if let Some(path) = path_to_output_rrd {
            let writer = BufWriter::new(File::create(path)?);
            process_messages(
                &rewrites,
                *continue_on_error,
                writer,
                &rx,
                drop_blueprint_activation_cmds,
            )?;
        } else {
            let stdout = std::io::stdout();
            let lock = stdout.lock();
            let writer = BufWriter::new(lock);
            process_messages(
                &rewrites,
                *continue_on_error,
                writer,
                &rx,
                drop_blueprint_activation_cmds,
            )?;
        }

        Ok(())
    }
}

fn process_messages<W: std::io::Write>(
    rewrites: &Rewrites,
    continue_on_error: bool,
    writer: W,
    receiver: &Receiver<(InputSource, anyhow::Result<Msg>)>,
    drop_blueprint_activation_cmds: bool,
) -> anyhow::Result<()> {
    re_log::info!("processing input…");
    let mut num_total_msgs = 0;
    let mut num_unexpected_msgs = 0;
    let mut num_blueprint_activations = 0;

    // TODO(grtlr): encoding should match the original (just like in `rrd stats`).
    let options = re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = re_build_info::CrateVersion::LOCAL;
    let mut encoder = DroppableEncoder::new(version, options, writer)?;

    while let Ok((_input, res)) = receiver.recv() {
        let mut is_success = true;

        match res {
            Ok(mut msg) => {
                num_total_msgs += 1;

                if matches!(&msg, Msg::BlueprintActivationCommand(_))
                    && drop_blueprint_activation_cmds
                {
                    num_blueprint_activations += 1;
                    continue;
                }

                match &mut msg {
                    Msg::SetStoreInfo(SetStoreInfo {
                        info: Some(StoreInfo { store_id, .. }),
                        ..
                    })
                    | Msg::ArrowMsg(ArrowMsg { store_id, .. }) => {
                        if let Some(target_store_id) = store_id {
                            if let Some(recording_id) = &rewrites.recording_id {
                                target_store_id.recording_id = recording_id.clone();
                            }

                            if let Some(application_id) = &rewrites.application_id {
                                target_store_id.application_id = Some(application_id.clone());
                            }
                        }
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
        "Processed {num_total_msgs} messages, dropped {num_blueprint_activations} blueprint activations, and encountered {num_unexpected_msgs} unexpected messages."
    );
    Ok(())
}
