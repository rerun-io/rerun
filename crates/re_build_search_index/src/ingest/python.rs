use std::fs::read_to_string;

use cargo_metadata::semver::Version;
use itertools::Itertools;
use serde::Deserialize;

use super::Context;

const RERUN_SDK: &str = "rerun_sdk";

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("python");
    let section_table: SectionTable = serde_json::from_str(&read_to_string(
        ctx.workspace_root()
            .join("rerun_py/docs/section_table.json"),
    )?)?;

    // 1. bring back griffe stuff
    // 2. read docstrings using griffe stuff
    // 3. push documents with title, content, url, yippeee

    for section in section_table {
        let slug = section
            .title
            .trim()
            .to_owned()
            .to_ascii_lowercase()
            .split_ascii_whitespace()
            .join("_");

        for func in section.func_list {
            todo!()
        }

        for class in section.class_list {
            todo!()
        }
    }

    Ok(())
}

type SectionTable = Vec<Section>;

#[derive(Deserialize)]
struct Section {
    title: String,
    #[serde(default = "Vec::new")]
    func_list: Vec<String>,
    #[serde(default = "Vec::new")]
    class_list: Vec<String>,
    mod_path: String,
}

fn item_url(version: &Version, slug: &str, mod_path: &str, item_path: &str) -> String {
    format!("https://ref.rerun.io/docs/python/{version}/common/{slug}/#{mod_path}.{item_path}")
}
