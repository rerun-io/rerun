mod lib;

use argh::FromArgs;
use cargo_metadata::camino::Utf8PathBuf;

use lib::{build, default_build_dir, Profile, Target};

/// Build the web-viewer.
#[derive(FromArgs)]
#[argh(subcommand, name = "build-web-viewer")]
pub struct Args {
    /// compile for release and run wasm-opt.
    ///
    /// Mutually exclusive with `--debug`.
    /// NOTE: --release also removes debug symbols which are otherwise useful for in-browser profiling.
    #[argh(switch)]
    release: bool,

    /// compile for debug and don't run wasm-opt.
    ///
    /// Mutually exclusive with `--release`.
    #[argh(switch)]
    debug: bool,

    /// keep debug symbols, even in release builds.
    /// This gives better callstacks on panics, and also allows for in-browser profiling of the Wasm.
    #[argh(switch, short = 'g')]
    debug_symbols: bool,

    /// if set, will build the module target instead of the browser target.
    #[argh(switch)]
    module: bool,

    /// set the output directory. This is a path relative to the cargo workspace root.
    #[argh(option, short = 'o', long = "out")]
    build_dir: Option<Utf8PathBuf>,
}

pub fn main(args: Args) -> anyhow::Result<()> {
    let profile = if args.release && !args.debug {
        Profile::Release
    } else if !args.release && args.debug {
        Profile::Debug
    } else {
        return Err(anyhow::anyhow!(
            "Exactly one of --release or --debug must be set"
        ));
    };

    let target = if args.module {
        Target::Module
    } else {
        Target::Browser
    };
    let build_dir = args.build_dir.unwrap_or_else(default_build_dir);

    build(profile, args.debug_symbols, target, &build_dir)
}
