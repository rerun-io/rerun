//! Example of an external data-loader executable plugin for the Rerun Viewer.

use rerun::MediaType;

const USAGE: &str = "
This is an example executable data-loader plugin for the Rerun Viewer.

It will log Rust source code files as markdown documents.
To try it out, install it in your $PATH (`cargo install --path . -f`), then open a Rust source file with Rerun (`rerun file.rs`).

USAGE:
  rerun-loader-rust-file [OPTIONS] FILEPATH

FLAGS:
  -h, --help                    Prints help information

OPTIONS:
  --recording-id RECORDING_ID   ID of the shared recording

ARGS:
  <FILEPATH>
";

#[allow(clippy::exit)]
fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(1);
}

// The Rerun Viewer will always pass these two pieces of information:
// 1. The path to be loaded, as a positional arg.
// 2. A shared recording ID, via the `--recording-id` flag.
//
// It is up to you whether you make use of that shared recording ID or not.
// If you use it, the data will end up in the same recording as all other plugins interested in
// that file, otherwise you can just create a dedicated recording for it. Or both.
struct Args {
    filepath: std::path::PathBuf,
    recording_id: Option<String>,
}

impl Args {
    fn from_env() -> Result<Self, pico_args::Error> {
        let mut pargs = pico_args::Arguments::from_env();
        Ok(Self {
            filepath: pargs.free_from_str()?,
            recording_id: pargs.opt_value_from_str("--recording-id")?,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Ok(args) = Args::from_env() else {
        usage();
    };

    let is_file = args.filepath.is_file();
    let is_rust_file = args
        .filepath
        .extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        == "rs";

    // We're not interested: just exit silently.
    // Don't return an error, as that would show up to the end user in the Rerun Viewer!
    if !(is_file && is_rust_file) {
        return Ok(());
    }

    let rec = {
        let mut rec = rerun::RecordingStreamBuilder::new("rerun_example_external_data_loader");
        if let Some(recording_id) = args.recording_id {
            rec = rec.recording_id(recording_id);
        };

        // The most important part of this: log to standard output so the Rerun Viewer can ingest it!
        rec.stdout()?
    };

    let body = std::fs::read_to_string(&args.filepath)?;
    let text = format!("## Some Rust code\n```rust\n{body}\n```\n");

    rec.log_timeless(
        rerun::EntityPath::from_file_path(&args.filepath),
        &rerun::TextDocument::new(text).with_media_type(MediaType::MARKDOWN),
    )?;

    Ok(())
}
