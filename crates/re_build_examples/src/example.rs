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
}

#[derive(Default, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Main,
    Nightly,
}

impl Channel {
    pub fn includes(self, other: Channel) -> bool {
        match self {
            Channel::Main => matches!(other, Channel::Main),

            // Include all `main` examples in `nightly`
            Channel::Nightly => matches!(other, Channel::Main | Channel::Nightly),
        }
    }

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

                let Some(channel) = readme.channel else {
                    eprintln!("{name:?}: skipped");
                    continue;
                };

                if !self.includes(channel) {
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
    type Err = InvalidChannelName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => Ok(Self::Main),
            "nightly" => Ok(Self::Nightly),
            _ => Err(InvalidChannelName),
        }
    }
}

#[derive(Debug)]
pub struct InvalidChannelName;

impl std::error::Error for InvalidChannelName {}

impl Display for InvalidChannelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid channel name")
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
    channel: Option<Channel>,

    #[serde(default)]
    build_args: Vec<String>,
}

fn parse_frontmatter<P: AsRef<Path>>(path: P) -> anyhow::Result<Option<Frontmatter>> {
    const START: &str = "<!--[metadata]";
    const END: &str = "-->";

    let path = path.as_ref();
    let content = std::fs::read_to_string(path)?;

    let Some(start) = content.find(START) else {
        return Ok(None);
    };
    let start = start + START.len();

    let Some(end) = content[start..].find(END) else {
        anyhow::bail!("{:?} has invalid frontmatter: missing --> terminator", path);
    };
    let end = start + end;

    toml::from_str(content[start..end].trim())
        .map(Some)
        .map_err(|err| {
            anyhow::anyhow!(
                "Failed to parse TOML metadata of {:?}: {err}",
                path.parent().unwrap().file_name().unwrap()
            )
        })
}
