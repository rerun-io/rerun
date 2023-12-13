//! Example data-loader binary plugin for the Rerun Viewer.

use rerun::MediaType;

const USAGE: &str = "
This is an example executable data-loader plugin for the Rerun Viewer.

It will log Rust source code files as markdown documents.
To try it out, install it in your path (`cargo install --path . -f`), then open a Rust source file with Rerun (`rerun file.rs`).

USAGE:
  rerun-loader-rs [OPTIONS] FILEPATH

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

struct Args {
    filepath: std::path::PathBuf,
    application_id: Option<String>,
    recording_id: Option<String>,
}

impl Args {
    fn from_env() -> Result<Self, pico_args::Error> {
        let mut pargs = pico_args::Arguments::from_env();
        Ok(Self {
            filepath: pargs.free_from_str()?,
            application_id: pargs.opt_value_from_str("--application-id")?,
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

    // Silently exit if we don't support the output.
    // Don't return an error, as that would show up to the end user in the Rerun Viewer!
    if !(is_file && is_rust_file) {
        return Ok(());
    }

    let rec = {
        let mut rec =
            // rerun::RecordingStreamBuilder::new(args.filepath.to_string_lossy().to_string());
            rerun::RecordingStreamBuilder::new(args.application_id.unwrap_or_else(|| args.filepath.to_string_lossy().to_string()));
        if let Some(recording_id) = args.recording_id {
            rec = rec.recording_id(recording_id);
        };
        rec.stdout()?
    };

    let body = std::fs::read_to_string(&args.filepath)?;

    let text = format!(
        "
## Some Rust code

```rust
{body}
```
"
    );

    rec.log(
        rerun::EntityPath::from_file_path(&args.filepath),
        &rerun::TextDocument::new(text).with_media_type(MediaType::MARKDOWN),
    )?;

    Ok(())
}
