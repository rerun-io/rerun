use argh::FromArgs;

mod build_examples;

#[derive(FromArgs)]
#[argh(subcommand)]
enum Commands {
    BuildExamples(build_examples::Args),
    // TODO: etc.
}

#[derive(FromArgs)]
/// Top-level command.
struct TopLevel {
    #[argh(subcommand)]
    cmd: Commands,
}

fn main() -> anyhow::Result<()> {
    let args: TopLevel = argh::from_env();

    match args.cmd {
        Commands::BuildExamples(args) => build_examples::main(args),
    }
}
