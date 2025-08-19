use anyhow::Context as _;
use crossbeam::channel::Receiver;
use itertools::Either;
use re_build_info::CrateVersion;
use re_log_types::{EntityPathFilter, EntityPathSubs, LogMsg, ResolvedEntityPathFilter};

use crate::commands::{read_rrd_streams_from_file_or_stdin, stdio::InputSource};

#[derive(Debug, Clone, clap::Parser)]
pub struct TransformCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// TODO: document this.
    #[arg(short = 't', long = "transform")]
    transforms: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.rrd")]
    path_to_output_rrd: Option<String>,
    //
    // If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,
}

struct TransformRule {
    path_expression: ResolvedEntityPathFilter,
    sql_expression: String,
}

impl TransformRule {
    fn parse(s: &str) -> anyhow::Result<Self> {
        let (path_expression, sql_expression) = s
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("expected '='"))?;

        let path_expression = EntityPathFilter::parse_forgiving(path_expression);
        let path_expression = path_expression.resolve_forgiving(&EntityPathSubs::empty());

        Ok(Self {
            path_expression,
            sql_expression: sql_expression.to_owned(),
        })
    }
}

impl TransformCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            transforms,
            path_to_output_rrd,
            continue_on_error,
        } = self;

        let transforms = transforms
            .iter()
            .map(|s| TransformRule::parse(s))
            .collect::<Result<Vec<_>, _>>()
            .context("parsing transform rules")?;

        let (rx, _) = read_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        process_messages(
            path_to_output_rrd.clone(),
            &rx,
            &transforms,
            *continue_on_error,
        )
    }
}

fn process_messages(
    path_to_output_rrd: Option<String>,
    receiver: &Receiver<(InputSource, anyhow::Result<LogMsg>)>,
    transforms: &[TransformRule],
    continue_on_error: bool,
) -> anyhow::Result<()> {
    re_log::info!("processing input…");

    // TODO(cmc): might want to make this configurable at some point.
    let (tx_encoder, rx_encoder) = crossbeam::channel::bounded(100);
    let encoding_handle = spawn_encode_thread(path_to_output_rrd, rx_encoder);

    // TODO: deeper pipeline (arbitrary number of transform & encoder tasks)
    for (_source, res) in receiver {
        let mut is_success = true;

        match res {
            Ok(mut msg) => {
                if let re_log_types::LogMsg::ArrowMsg(_store_id, msg) = &mut msg {
                    let record_batch = &mut msg.batch;

                    dbg!(&record_batch.schema().metadata.get("rerun:entity_path"));
                }

                // TODO: skip empty messages.
                tx_encoder.send(msg).ok();
            }

            Err(err) => {
                re_log::error!(err = re_error::format(err));
                is_success = false;
            }
        }

        if !continue_on_error && !is_success {
            anyhow::bail!(
                "one or more IO and/or decoding failures in the input stream (check logs)"
            )
        }
    }

    std::mem::drop(tx_encoder);
    let _rrd_out_size = encoding_handle
        .context("couldn't spawn IO thread")?
        .join()
        .map_err(|err| anyhow::anyhow!("Unknown error: {err:?}"))??; // NOLINT: there is no `Display` for this `err`

    // TODO: print some stats.

    Ok(())
}

// TODO: same thing as in filter.rs and maybe others.
fn spawn_encode_thread(
    path_to_output_rrd: Option<String>,
    rx_encoder: Receiver<LogMsg>,
) -> Result<std::thread::JoinHandle<Result<u64, anyhow::Error>>, std::io::Error> {
    std::thread::Builder::new()
        .name("rerun-rrd-transform".to_owned())
        .spawn(move || -> anyhow::Result<u64> {
            use std::io::Write as _;

            let mut rrd_out = if let Some(path) = path_to_output_rrd.as_ref() {
                Either::Left(std::io::BufWriter::new(
                    std::fs::File::create(path).with_context(|| format!("{path:?}"))?,
                ))
            } else {
                Either::Right(std::io::BufWriter::new(std::io::stdout().lock()))
            };

            let mut encoder = {
                // TODO(cmc): encoding options & version should match the original.
                let version = CrateVersion::LOCAL;
                let options = re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED;
                re_log_encoding::encoder::DroppableEncoder::new(version, options, &mut rrd_out)
                    .context("couldn't init encoder")?
            };

            let mut size_bytes = 0;
            for msg in rx_encoder {
                size_bytes += encoder.append(&msg).context("encoding failure")?;
            }

            drop(encoder);
            rrd_out.flush().context("couldn't flush output")?;

            Ok(size_bytes)
        })
}
