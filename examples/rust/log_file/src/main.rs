//! Demonstrates how to log any file from the SDK using the `DataLoader` machinery.
//!
//! See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
//!
//! Usage:
//! ```
//! cargo run -p log_file -- examples/assets
//! ```

use rerun::external::re_log;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    // Log the contents of the file directly (files only -- not supported by external loaders).
    #[clap(long, default_value = "false")]
    from_contents: bool,

    /// The filepaths to be loaded and logged.
    filepaths: Vec<std::path::PathBuf>,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_log_file")?;
    run(&rec, &args)?;

    Ok(())
}

fn run(rec: &rerun::RecordingStream, args: &Args) -> anyhow::Result<()> {
    let prefix = Some("log_file_example".into());

    for filepath in &args.filepaths {
        let filepath = filepath.as_path();

        if !args.from_contents {
            // Either log the file using its path…
            rec.log_file_from_path(filepath, prefix.clone(), None, true /* static */)?;
        } else {
            // …or using its contents if you already have them loaded for some reason.
            if filepath.is_file() {
                let contents = std::fs::read(filepath)?;
                rec.log_file_from_contents(
                    filepath,
                    std::borrow::Cow::Borrowed(&contents),
                    prefix.clone(),
                    None,
                    true, /* static */
                )?;
            }
        }
    }

    Ok(())
}
