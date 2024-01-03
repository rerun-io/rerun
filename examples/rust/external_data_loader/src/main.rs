//! Example of an external data-loader executable plugin for the Rerun Viewer.

use rerun::{
    external::re_data_source::extension, MediaType, EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE,
};

// The Rerun Viewer will always pass these two pieces of information:
// 1. The path to be loaded, as a positional arg.
// 2. A shared recording ID, via the `--recording-id` flag.
//
// It is up to you whether you make use of that shared recording ID or not.
// If you use it, the data will end up in the same recording as all other plugins interested in
// that file, otherwise you can just create a dedicated recording for it. Or both.

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
#[derive(argh::FromArgs)]
struct Args {
    #[argh(positional)]
    filepath: std::path::PathBuf,

    /// optional ID of the shared recording
    #[argh(option)]
    recording_id: Option<String>,
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
        let mut rec = rerun::RecordingStreamBuilder::new("rerun_example_external_data_loader");
        if let Some(recording_id) = args.recording_id {
            rec = rec.recording_id(recording_id);
        };

        // The most important part of this: log to standard output so the Rerun Viewer can ingest it!
        rec.stdout()?
    };

    rec.log_timeless(
        rerun::EntityPath::from_file_path(&args.filepath),
        &rerun::TextDocument::new(text).with_media_type(MediaType::MARKDOWN),
    )?;

    Ok::<_, anyhow::Error>(())
}
