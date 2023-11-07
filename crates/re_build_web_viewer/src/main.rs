use cargo_metadata::camino::Utf8PathBuf;
use re_build_web_viewer::{default_build_dir, Backend, Profile, Target};
use std::process::ExitCode;

struct Opts {
    profile: Option<Profile>,
    backend: Backend,
    target: Target,
    build_dir: Utf8PathBuf,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            profile: None,
            backend: Backend::WebGL,
            target: Target::Browser,
            build_dir: default_build_dir(),
        }
    }
}

fn main() -> ExitCode {
    let mut opts = Opts::default();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "--debug" => {
                assert!(
                    opts.profile.is_none(),
                    "Can't set both --release and --debug"
                );
                opts.profile = Some(Profile::Debug);
            }
            "--release" => {
                assert!(
                    opts.profile.is_none(),
                    "Can't set both --release and --debug"
                );
                opts.profile = Some(Profile::Release);
            }
            "--webgpu" => {
                opts.backend = Backend::WebGPU;
            }
            "-o" | "--out" => match args.next() {
                Some(value) if !value.starts_with('-') => {
                    opts.build_dir = Utf8PathBuf::from(value);
                }
                _ => panic!("expected path after {arg}"),
            },
            "--module" => {
                opts.target = Target::Module;
            }
            _ => {
                print_help();
                return ExitCode::FAILURE;
            }
        }
    }

    let Some(release) = opts.profile else {
        eprintln!("You need to pass either --debug or --release");
        return ExitCode::FAILURE;
    };

    if let Err(err) =
        re_build_web_viewer::build(release, opts.backend, opts.target, &opts.build_dir)
    {
        eprintln!("Failed to build web viewer: {}", re_error::format(err));
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_help() {
    eprintln!(
        r"Build the web-viewer.

  --help:     Print this help text
  --debug:    Build a debug binary
  --release:  Compile for release, and run wasm-opt.
              NOTE: --release also removes debug symbols which are otherwise useful for in-browser profiling.
  --webgpu:   Enable WebGPU support (experimental). If not set the viewer will use WebGL instead.
  -o, --out:  Set the output directory. This is a path relative to the cargo workspace root.
"
    );
}
