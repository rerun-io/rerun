use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufWriter;

use clap::Subcommand;
use re_log_encoding::Encoder;
use re_log_types::{LogMsg, RecordingId};
use re_mcap::{LayerIdentifier, SelectedLayers};
use re_sdk::external::re_data_loader::McapLoader;
use re_sdk::{ApplicationId, DataLoader, DataLoaderSettings, LoadedData};

#[derive(Debug, Clone, clap::Parser)]
pub struct ConvertCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_mcap: String,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.rrd")]
    path_to_output_rrd: Option<String>,

    /// If set, specifies the application id of the output.
    #[clap(long = "application-id")]
    application_id: Option<String>,

    /// Specifies which layers to apply during conversion.
    #[clap(short = 'l', long = "layer")]
    selected_layers: Vec<String>,

    /// Disable using the raw layer as a fallback for unsupported channels.
    /// By default, channels that cannot be handled by semantic layers (protobuf, ROS2)
    /// will be processed by the raw layer.
    #[clap(long = "disable-raw-fallback")]
    disable_raw_fallback: bool,

    /// If set, specifies the recording id of the output.
    ///
    /// When this flag is set and multiple input .rdd files are specified,
    /// blueprint activation commands will be dropped from the resulting
    /// output.
    #[clap(long = "recording-id")]
    recording_id: Option<String>,
}

impl ConvertCommand {
    fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_mcap,
            path_to_output_rrd,
            application_id,
            recording_id,
            selected_layers,
            disable_raw_fallback,
        } = self;

        let start_time = std::time::Instant::now();

        let application_id = application_id
            .to_owned()
            .map(ApplicationId::from)
            .unwrap_or_else(|| ApplicationId::from(path_to_input_mcap.clone()));

        let recording_id = recording_id
            .to_owned()
            .map(RecordingId::from)
            .unwrap_or_else(RecordingId::random);

        let selected_layers = if selected_layers.is_empty() {
            SelectedLayers::All
        } else {
            SelectedLayers::Subset(
                selected_layers
                    .iter()
                    .cloned()
                    .map(LayerIdentifier::from)
                    .collect(),
            )
        };

        let loader: &dyn DataLoader =
            &McapLoader::with_raw_fallback(selected_layers, !*disable_raw_fallback);

        // TODO(#10862): This currently loads the entire file into memory.
        let (tx, rx) = crossbeam::channel::bounded::<LoadedData>(1024);
        loader.load_from_path(
            &DataLoaderSettings {
                application_id: Some(application_id),
                recording_id,
                opened_store_id: None,
                force_store_info: false,
                entity_path_prefix: None,
                timepoint: None,
            },
            path_to_input_mcap.into(),
            tx,
        )?;

        if let Some(path) = path_to_output_rrd {
            let writer = BufWriter::new(File::create(path)?);
            process_mcap(writer, &rx)?;
        } else {
            let stdout = std::io::stdout();
            let lock = stdout.lock();
            let writer = BufWriter::new(lock);
            process_mcap(writer, &rx)?;
        }

        re_log::info!("Processing took {}s", start_time.elapsed().as_secs());

        Ok(())
    }
}

/// Manipulate the contents of .mcap files.
#[derive(Debug, Clone, Subcommand)]
pub enum McapCommands {
    /// Convert an .mcap file to an .rrd
    Convert(ConvertCommand),
}

impl McapCommands {
    pub fn run(&self) -> anyhow::Result<()> {
        match self {
            Self::Convert(cmd) => cmd.run(),
        }
    }
}

fn process_mcap<W: std::io::Write>(
    writer: W,
    receiver: &crossbeam::channel::Receiver<LoadedData>,
) -> anyhow::Result<()> {
    let mut num_total_msgs = 0;
    let mut topics = BTreeSet::new();
    let options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = re_build_info::CrateVersion::LOCAL;
    let mut encoder = Encoder::new_eager(version, options, writer)?;

    while let Ok(res) = receiver.recv() {
        num_total_msgs += 1;

        let log_msg = match res {
            LoadedData::LogMsg(_, log_msg) => log_msg,
            LoadedData::Chunk(_, store_id, chunk) => {
                topics.insert(chunk.entity_path().clone());
                let arrow_msg = chunk.to_arrow_msg()?;
                LogMsg::ArrowMsg(store_id, arrow_msg)
            }
            LoadedData::ArrowMsg(_, store_id, arrow_msg) => LogMsg::ArrowMsg(store_id, arrow_msg),
        };
        encoder.append(&log_msg)?;
    }

    re_log::info_once!("Processed {num_total_msgs} messages.");
    re_log::info_once!("Entities: {topics:#?}");
    Ok(())
}
