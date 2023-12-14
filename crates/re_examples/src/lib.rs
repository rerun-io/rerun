//! Example collection and parsing.

use std::fmt::Display;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

pub struct Example {
    pub name: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub thumbnail_url: String,
    pub thumbnail_dimensions: [u64; 2],
    pub script_path: PathBuf,
    pub script_args: Vec<String>,
    pub kind: ExampleKind,
}

impl ExampleKind {
    #[allow(clippy::match_like_matches_macro)] // harder to read
    pub fn included_in(&self, channel: Channel) -> bool {
        match (channel, self) {
            (Channel::Nightly, ExampleKind::Nightly | ExampleKind::Demo) => true,
            (Channel::Main, ExampleKind::Demo) => true,
            _ => false,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum Channel {
    #[default]
    Main,
    Nightly,
}

impl Channel {
    pub fn examples(self) -> anyhow::Result<Vec<Example>> {
        let mut examples = vec![];
        let dir = Path::new("examples/python");
        if !dir.exists() {
            anyhow::bail!("Failed to find {}", dir.display())
        }
        if !dir.is_dir() {
            anyhow::bail!("{} is not a directory", dir.display())
        }

        let folders: std::collections::BTreeMap<String, std::fs::DirEntry> =
            std::fs::read_dir(dir)?
                .filter_map(Result::ok)
                .map(|folder| {
                    let name = folder.file_name().to_string_lossy().to_string();
                    (name, folder)
                })
                .collect();

        for (name, folder) in folders {
            let metadata = folder.metadata()?;
            let readme = folder.path().join("README.md");
            if metadata.is_dir() && readme.exists() {
                let readme = parse_frontmatter(readme)?;
                let Some(readme) = readme else {
                    eprintln!("{name:?}: skipped - MISSING FRONTMATTER");
                    continue;
                };

                let kind = ExampleKind::infer(readme.demo, readme.nightly);
                if !kind.included_in(self) {
                    eprintln!("{name:?}: skipped");
                    continue;
                }

                eprintln!("{name:?}: added");
                examples.push(Example {
                    name,
                    title: readme.title,
                    description: readme.description,
                    tags: readme.tags,
                    thumbnail_url: readme.thumbnail,
                    thumbnail_dimensions: readme.thumbnail_dimensions,
                    script_path: folder.path().join("main.py"),
                    script_args: readme.build_args,
                    kind,
                });
            }
        }

        if examples.is_empty() {
            anyhow::bail!("No examples found in {}", dir.display())
        }

        examples.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(examples)
    }
}

impl Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Channel::Main => "main",
            Channel::Nightly => "nightly",
        };
        f.write_str(s)
    }
}

impl FromStr for Channel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => Ok(Self::Main),
            "nightly" => Ok(Self::Nightly),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy)]
pub enum ExampleKind {
    Ignore,
    Demo,
    Nightly,
}

impl ExampleKind {
    fn infer(demo: bool, nightly: bool) -> Self {
        if demo && nightly {
            Self::Nightly
        } else if demo {
            Self::Demo
        } else {
            Self::Ignore
        }
    }
}

#[derive(serde::Deserialize)]
struct Frontmatter {
    #[serde(default)]
    title: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    thumbnail: String,
    #[serde(default)]
    thumbnail_dimensions: [u64; 2],
    #[serde(default)]
    demo: bool,
    #[serde(default)]
    nightly: bool,
    #[serde(default)]
    build_args: Vec<String>,
}

fn parse_frontmatter<P: AsRef<Path>>(path: P) -> anyhow::Result<Option<Frontmatter>> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)?;
    let content = content.replace('\r', ""); // Windows, god damn you
    re_build_tools::rerun_if_changed(path);
    let Some(content) = content.strip_prefix("---\n") else {
        return Ok(None);
    };
    let Some(end) = content.find("---") else {
        anyhow::bail!("{:?} has invalid frontmatter", path);
    };
    Ok(Some(serde_yaml::from_str(&content[..end]).map_err(
        |e| {
            anyhow::anyhow!(
                "failed to read {:?}: {e}",
                path.parent().unwrap().file_name().unwrap()
            )
        },
    )?))
}
