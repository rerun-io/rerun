//! Example of an external SVG data-loader executable plugin for the Rerun Viewer.

use rerun::{
    external::{image::ImageFormat, re_data_source::extension},
    EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE,
};

// The Rerun Viewer will always pass these two pieces of information:
// 1. The path to be loaded, as a positional arg.
// 2. A shared recording ID, via the `--recording-id` flag.
//
// It is up to you whether you make use of that shared recording ID or not.
// If you use it, the data will end up in the same recording as all other plugins interested in
// that file, otherwise you can just create a dedicated recording for it. Or both.

/// This is an example executable data-loader plugin for the Rerun Viewer.
///
/// It will log Rust source code files as markdown documents.
/// To try it out, install it in your $PATH (`cargo install --path . -f`), then open
/// Rust source file with Rerun (`rerun file.rs`).
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
    let is_graphviz = extension(&args.filepath) == "dot";

    // Inform the Rerun Viewer that we do not support that kind of file.
    if !is_file || !is_graphviz {
        #[allow(clippy::exit)]
        std::process::exit(EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE);
    }

    let rec = {
        let mut rec = rerun::RecordingStreamBuilder::new("rerun_example_external_svg_data_loader");
        if let Some(recording_id) = args.recording_id {
            rec = rec.recording_id(recording_id);
        };

        // The most important part of this: log to standard output so the Rerun Viewer can ingest it!
        rec.stdout()?
    };

    // dot -Tpng -Gsize=40,40\! -Gdpi=300 -Granksep=5 deps.dot > deps.png
    use std::process::Command;
    let output = Command::new("dot")
        .args([
            "-Tpng",
            "-Gsize=40,40\\!",
            "-Gdpi=300",
            "-Granksep=5",
            args.filepath.to_string_lossy().as_ref(),
        ])
        .output()?;

    assert!(output.status.success());

    rec.log_timeless(
        rerun::EntityPath::from_file_path(&args.filepath),
        &rerun::Image::from_file_contents(output.stdout, Some(ImageFormat::Png))?,
    )?;

    Ok::<_, anyhow::Error>(())
}
