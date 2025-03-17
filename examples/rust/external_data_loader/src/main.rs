//! Example of an external data-loader executable plugin for the Rerun Viewer.

use rerun::{TimeCell, EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE};

// The Rerun Viewer will always pass at least these two pieces of information:
// 1. The path to be loaded, as a positional arg.
// 2. A shared recording ID, via the `--recording-id` flag.
//
// It is up to you whether you make use of that shared recording ID or not.
// If you use it, the data will end up in the same recording as all other plugins interested in
// that file, otherwise you can just create a dedicated recording for it. Or both.
//
// Check out `re_data_source::DataLoaderSettings` documentation for an exhaustive listing of
// the available CLI parameters.

/// This is an example executable data-loader plugin for the Rerun Viewer.
/// Any executable on your `$PATH` with a name that starts with [`rerun-loader-`] will be
/// treated as an external data-loader.
///
/// This particular one will log Rust source code files as markdown documents, and return a
/// special exit code to indicate that it doesn't support anything else.
///
/// To try it out, install it in your $PATH (`cargo install --path . -f`), then open a
/// Rust source file with Rerun (`rerun file.rs`).
///
/// [`rerun-loader-`]: `rerun::EXTERNAL_DATA_LOADER_PREFIX`
#[derive(argh::FromArgs, Debug)]
struct Args {
    #[argh(positional)]
    filepath: std::path::PathBuf,

    /// optional recommended ID for the application
    #[argh(option)]
    application_id: Option<String>,

    /// optional recommended ID for the recording
    #[argh(option)]
    recording_id: Option<String>,

    /// optional prefix for all entity paths
    #[argh(option)]
    entity_path_prefix: Option<String>,

    /// optionally mark data to be logged statically
    #[argh(arg_name = "static", switch)]
    static_: bool,

    /// optional timestamps to log at (e.g. `--time sim_time=1709203426`) (repeatable)
    #[argh(option)]
    time: Vec<String>,

    /// optional sequences to log at (e.g. `--sequence sim_frame=42`) (repeatable)
    #[argh(option)]
    sequence: Vec<String>,
}

fn extension(path: &std::path::Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string()
}

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    let is_file = args.filepath.is_file();
    let is_rust_file = extension(&args.filepath) == "rs";

    // Inform the Rerun Viewer that we do not support that kind of file.
    if !is_file || !is_rust_file {
        #[allow(clippy::exit)]
        std::process::exit(EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE);
    }

    let body = std::fs::read_to_string(&args.filepath)?;
    let text = format!("## Some Rust code\n```rust\n{body}\n```\n");

    let rec = {
        let mut rec = rerun::RecordingStreamBuilder::new(
            args.application_id
                .as_deref()
                .unwrap_or("rerun_example_external_data_loader"),
        );
        if let Some(recording_id) = args.recording_id.as_ref() {
            rec = rec.recording_id(recording_id);
        };

        // The most important part of this: log to standard output so the Rerun Viewer can ingest it!
        rec.stdout()?
    };

    // TODO(#3841): In the future, we will introduce so-called stateless APIs that allow logging
    // data at a specific timepoint without having to modify the global stateful clock.
    rec.set_timepoint(timepoint_from_args(&args)?);

    let entity_path_prefix = args
        .entity_path_prefix
        .map_or_else(|| rerun::EntityPath::new(vec![]), rerun::EntityPath::from);

    rec.log_with_static(
        entity_path_prefix.join(&rerun::EntityPath::from_file_path(&args.filepath)),
        args.static_,
        &rerun::TextDocument::from_markdown(text),
    )?;

    Ok::<_, anyhow::Error>(())
}

fn timepoint_from_args(args: &Args) -> anyhow::Result<rerun::TimePoint> {
    let mut timepoint = rerun::TimePoint::default();
    {
        for time_str in &args.time {
            let Some((timeline_name, time)) = time_str.split_once('=') else {
                continue;
            };
            timepoint.insert_cell(
                timeline_name,
                TimeCell::from_duration_nanos(time.parse::<i64>()?),
            );
        }

        for seq_str in &args.sequence {
            let Some((seqline_name, seq)) = seq_str.split_once('=') else {
                continue;
            };
            timepoint.insert_cell(
                seqline_name,
                rerun::TimeCell::from_sequence(seq.parse::<i64>()?),
            );
        }
    }
    Ok(timepoint)
}
