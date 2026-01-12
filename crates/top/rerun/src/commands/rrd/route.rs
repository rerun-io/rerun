use std::fs::File;
use std::io::BufWriter;

use crossbeam::channel::Receiver;
use re_log_encoding::Encoder;
use re_protos::common::v1alpha1::ApplicationId;
use re_protos::log_msg::v1alpha1::log_msg::Msg;
use re_protos::log_msg::v1alpha1::{ArrowMsg, BlueprintActivationCommand, SetStoreInfo, StoreInfo};

use crate::commands::read_raw_rrd_streams_from_file_or_stdin;
use crate::commands::stdio::InputSource;

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

    /// If set, this will compute an RRD footer with the appropriate manifest for the routed data.
    ///
    /// By default, `rerun rrd route` will always drop all existing RRD manifests when routing data,
    /// as doing so invalidates their contents.
    /// This flag makes it possible to recompute an RRD manifest for the routed data, but beware
    /// that it has to decode the data, which means it is A) much slower and B) will migrate
    /// the data to the latest Sorbet specification automatically.
    #[clap(long = "recompute-manifests", default_value_t = false)]
    recompute_manifests: bool,
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
            recompute_manifests,
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
                *recompute_manifests,
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
                *recompute_manifests,
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

#[expect(clippy::fn_params_excessive_bools)] // private function 🤷‍♂️
fn process_messages<W: std::io::Write>(
    recompute_manifests: bool,
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

    // Only used if recomputing manifests.
    let mut app_id_injector = re_log_encoding::CachingApplicationIdInjector::default();

    // TODO(grtlr): encoding should match the original (just like in `rrd stats`).
    let options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = re_build_info::CrateVersion::LOCAL;
    let mut encoder = Encoder::new_eager(version, options, writer)?;

    while let Ok((_input, res)) = receiver.recv() {
        let mut is_success = true;

        match res {
            Ok(mut msg) => {
                num_total_msgs += 1;

                #[expect(deprecated)]
                match &mut msg {
                    // This needs to come first, as an
                    Msg::BlueprintActivationCommand(_) if drop_blueprint_activation_cmds => {
                        num_blueprint_activations += 1;
                        continue;
                    }

                    Msg::SetStoreInfo(SetStoreInfo {
                        info:
                            Some(StoreInfo {
                                store_id,
                                application_id: _, // deprecated but not considered.
                                store_source: _,
                                store_version: _,
                            }),
                        row_id: _,
                    })
                    | Msg::BlueprintActivationCommand(BlueprintActivationCommand {
                        blueprint_id: store_id,
                        make_active: _,
                        make_default: _,
                    })
                    | Msg::ArrowMsg(ArrowMsg {
                        store_id,
                        chunk_id: _,
                        compression: _,
                        uncompressed_size: _,
                        encoding: _,
                        payload: _,
                        is_static: _,
                    }) => {
                        if let Some(target_store_id) = store_id {
                            if let Some(recording_id) = &rewrites.recording_id {
                                target_store_id.recording_id = recording_id.clone();
                            }

                            if let Some(application_id) = &rewrites.application_id {
                                target_store_id.application_id = Some(application_id.clone());
                            }
                        }
                    }

                    Msg::SetStoreInfo(SetStoreInfo {
                        row_id: _,
                        info: None,
                    }) => {
                        num_unexpected_msgs += 1;
                        is_success = false;
                        re_log::warn_once!(
                            "Encountered `SetStoreInfo` without `info` field: {:#?}",
                            msg
                        );
                    }
                }

                if recompute_manifests {
                    use re_log_encoding::ToApplication as _;
                    let msg = msg.to_application((&mut app_id_injector, None))?;
                    encoder.append(&msg)?;
                } else {
                    // Safety: we're just forwarding an existing message, we didn't change its payload
                    // in any meaningful way.
                    #[expect(unsafe_code)]
                    unsafe {
                        // Reminder: this will implicitly discard RRD footers.
                        encoder.append_transport(&msg)?;
                    }
                }
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
