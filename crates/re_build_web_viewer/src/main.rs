use std::process::ExitCode;

fn main() -> ExitCode {
    let mut release = None;
    let mut webgpu = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "--debug" => {
                assert!(release.is_none(), "Can't set both --release and --debug");
                release = Some(false);
            }
            "--release" => {
                assert!(release.is_none(), "Can't set both --release and --debug");
                release = Some(true);
            }
            "--webgpu" => {
                webgpu = true;
            }
            _ => {
                print_help();
                return ExitCode::FAILURE;
            }
        }
    }

    let Some(release) = release else {
        eprintln!("You need to pass either --debug or --release");
        return ExitCode::FAILURE;
    };

    if let Err(err) = re_build_web_viewer::build(release, webgpu) {
        eprintln!("Failed to build web viewer: {}", re_error::format(err));
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_help() {
    eprintln!(
        r"Build the web-viewer.

  --help:    Print this help text
  --debug:   Build a debug binary
  --release: Compile for release, and run wasm-opt.
             NOTE: --release also removes debug symbols which are otherwise useful for in-browser profiling.
  --webgpu:  Enable WebGPU support (experimental). If not set the viewer will use WebGL instead.
"
    );
}
