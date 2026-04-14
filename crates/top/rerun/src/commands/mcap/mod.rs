use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufWriter;

use clap::Subcommand;
use clap::builder::TypedValueParser as _;
use re_log_encoding::Encoder;
use re_log_types::{LogMsg, RecordingId, TimeType};
use re_mcap::{DecoderIdentifier, SelectedDecoders};
use re_sdk::external::re_importer::{McapImporter, supported_mcap_decoder_identifiers};
use re_sdk::{ApplicationId, ImportedData, Importer, ImporterSettings};

fn possible_timeline_types() -> impl clap::builder::TypedValueParser {
    clap::builder::PossibleValuesParser::new(["timestamp", "duration"]).map(|value: String| {
        match value.as_str() {
            "timestamp" => TimeType::TimestampNs,
            "duration" => TimeType::DurationNs,
            _ => unreachable!("PossibleValuesParser already validated the input"),
        }
    })
}

fn possible_decoders() -> clap::builder::PossibleValuesParser {
    static DECODER_IDS: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
        supported_mcap_decoder_identifiers(true)
            .into_iter()
            .map(|identifier| identifier.to_string())
            .collect()
    });
    clap::builder::PossibleValuesParser::new(
        DECODER_IDS.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

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

    /// Specifies which decoders to apply during conversion.
    #[clap(short = 'd', long = "decoder", value_parser = possible_decoders())]
    selected_decoders: Vec<String>,

    /// Disable using the raw decoder as a fallback for unsupported channels.
    /// By default, channels that cannot be handled by semantic decoders (protobuf, ROS2)
    /// will be processed by the raw decoder.
    #[clap(long = "disable-raw-fallback")]
    disable_raw_fallback: bool,

    /// If set, specifies the recording id of the output.
    ///
    /// When this flag is set and multiple input .rdd files are specified,
    /// blueprint activation commands will be dropped from the resulting
    /// output.
    #[clap(long = "recording-id")]
    recording_id: Option<String>,

    /// If set, an offset in nanoseconds to add to all timestamp timelines.
    ///
    /// This can be used to shift all timestamps of the MCAP file if they are not yet
    /// relative to the UNIX epoch.
    ///
    /// Duration and sequence timelines are not affected by this offset.
    #[clap(long = "timestamp-offset-ns")]
    timestamp_offset_ns: Option<i64>,

    /// The timeline type to use for timestamp timelines.
    ///
    /// "timestamp" (default) creates `TimestampNs` timelines (nanoseconds since Unix epoch).
    /// "duration" creates `DurationNs` timelines (nanosecond durations).
    #[clap(long = "timeline-type", value_parser = possible_timeline_types(), default_value = "timestamp")]
    timeline_type: TimeType,
}

impl ConvertCommand {
    fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_mcap,
            path_to_output_rrd,
            application_id,
            recording_id,
            selected_decoders,
            disable_raw_fallback,
            timestamp_offset_ns,
            timeline_type,
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

        let selected_decoders = if selected_decoders.is_empty() {
            SelectedDecoders::All
        } else {
            SelectedDecoders::Subset(
                selected_decoders
                    .iter()
                    .cloned()
                    .map(DecoderIdentifier::from)
                    .collect(),
            )
        };

        let importer: &dyn Importer =
            &McapImporter::new(&selected_decoders).with_raw_fallback(!*disable_raw_fallback);

        // TODO(#10862): This currently loads the entire file into memory.
        let (tx, rx) = crossbeam::channel::bounded::<ImportedData>(1024);
        importer.import_from_path(
            &ImporterSettings {
                application_id: Some(application_id),
                timestamp_offset_ns: *timestamp_offset_ns,
                timeline_type: *timeline_type,
                ..ImporterSettings::recommended(recording_id)
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
    receiver: &crossbeam::channel::Receiver<ImportedData>,
) -> anyhow::Result<()> {
    let mut num_total_msgs = 0;
    let mut topics = BTreeSet::new();
    let options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = re_build_info::CrateVersion::LOCAL;
    let mut encoder = Encoder::new_eager(version, options, writer)?;

    while let Ok(res) = receiver.recv() {
        num_total_msgs += 1;

        let log_msg = match res {
            ImportedData::LogMsg(_, log_msg) => log_msg,
            ImportedData::Chunk(_, store_id, chunk) => {
                topics.insert(chunk.entity_path().clone());
                let arrow_msg = chunk.to_arrow_msg()?;
                LogMsg::ArrowMsg(store_id, arrow_msg)
            }
            ImportedData::ArrowMsg(_, store_id, arrow_msg) => LogMsg::ArrowMsg(store_id, arrow_msg),
        };
        encoder.append(&log_msg)?;
    }

    re_log::info_once!("Processed {num_total_msgs} messages.");
    re_log::info_once!("Entities: {topics:#?}");
    Ok(())
}
