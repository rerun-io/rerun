use std::fs::read_to_string;
use std::io::BufReader;
use std::process::Command;

use cargo_metadata::semver::Version;
use itertools::Itertools;
use serde::Deserialize;

use crate::util::CommandExt as _;

use super::Context;

const RERUN_SDK: &str = "rerun_sdk";

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("python");

    progress.set_message("mkdocs build");
    Command::new("mkdocs")
        .with_arg("build")
        .with_arg("-f")
        .with_arg(ctx.workspace_root().join("rerun_py/mkdocs.yml"))
        .run()?;

    progress.set_message("sphobjinv convert");
    Command::new("sphobjinv")
        .with_args(["convert", "json"])
        .with_args([
            ctx.workspace_root().join("rerun_py/site/objects.inv"), // infile
            ctx.workspace_root().join("rerun_py/site/objects.json"), // outfile
        ])
        .with_args(["--overwrite", "--expand"])
        .run()?;

    progress.set_message("griffe dump");
    let griffe = Command::new("griffe")
        .with_args(["griffe", "dump", "rerun_sdk"])
        .spawn()?;
    let root = serde_json::from_reader(BufReader::new(griffe.stdout.unwrap()))?;

    Ok(())
}
