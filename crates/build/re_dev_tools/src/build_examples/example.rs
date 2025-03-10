//! Example collection and parsing.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context as _;

pub struct Example {
    /// Name of the folder it's stored in.
    pub name: String,
    pub title: String,
    pub dir: PathBuf,
    pub description: String,
    pub tags: Vec<String>,
    pub thumbnail_url: String,
    pub thumbnail_dimensions: [u64; 2],
    pub script_args: Vec<String>,
    pub readme_body: String,
    pub language: Language,
}

impl Example {
    fn exists(
        workspace_root: impl AsRef<Path>,
        name: &str,
        language: Language,
    ) -> anyhow::Result<bool> {
        Ok(workspace_root
            .as_ref()
            .join("examples")
            .join(language.examples_dir())
            .join(name)
            .try_exists()?)
    }

    pub fn load(
        workspace_root: impl AsRef<Path>,
        name: &str,
        language: Language,
    ) -> anyhow::Result<Option<Self>> {
        let workspace_root = workspace_root.as_ref();

        if !Self::exists(workspace_root, name, language)? {
            return Ok(None);
        }

        let dir = workspace_root
            .join("examples")
            .join(language.examples_dir())
            .join(name);
        let readme_path = dir.join("README.md");
        let Some((readme, body)) = Frontmatter::load(&readme_path).with_context(|| {
            format!(
                "loading example {}/{name} README.md",
                language.examples_dir().display()
            )
        })?
        else {
            anyhow::bail!("example {name:?} has no frontmatter");
        };
        Ok(Some(Self {
            name: name.to_owned(),
            title: readme.title,
            dir,
            description: readme.description,
            tags: readme.tags,
            thumbnail_url: readme.thumbnail,
            thumbnail_dimensions: readme.thumbnail_dimensions,
            script_args: readme.build_args,
            readme_body: body,
            language,
        }))
    }
}

#[derive(Clone, Copy)]
pub enum Language {
    Rust,
    Python,
    #[allow(dead_code)]
    C,
    Cpp,
}

impl Language {
    /// Path of the directory where examples for this language are stored,
    /// relative to `{workspace_root}/examples`.
    pub fn examples_dir(&self) -> &'static Path {
        match self {
            Self::Rust => Path::new("rust"),
            Self::Python => Path::new("python"),
            Self::C => Path::new("c"),
            Self::Cpp => Path::new("cpp"),
        }
    }

    /// Extension without the leading dot, e.g. `rs`.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Python => "py",
            Self::C => "c",
            Self::Cpp => "cpp",
        }
    }
}

#[derive(serde::Deserialize)]
pub struct ExamplesManifest {
    pub categories: BTreeMap<String, ExampleCategory>,
}

impl ExamplesManifest {
    /// Loads the `examples/manifest.toml` file.
    pub fn load(workspace_root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let manifest_toml = workspace_root
            .as_ref()
            .join("examples")
            .join("manifest.toml");
        let manifest =
            std::fs::read_to_string(manifest_toml).context("loading examples/manifest.toml")?;
        Ok(toml::from_str(&manifest)?)
    }
}

#[derive(serde::Deserialize)]
pub struct ExampleCategory {
    /// Used to sort categories in the `rerun.io/examples` navbar.
    #[allow(unused)]
    pub order: u64,

    /// `snake_case` name.
    pub title: String,

    /// Multi-line description.
    pub prelude: String,

    /// List of example names.
    ///
    /// `rerun.io/examples` attempts to search for these names under `examples/{language}`,
    /// where `language` is any of the languages we currently support.
    pub examples: Vec<String>,
}

#[derive(Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Our main examples, built on each PR
    Main,

    /// Examples built for each release, plus all `Main` examples.
    Release,

    /// Examples built nightly, plus all `Main` and `Release`.
    Nightly,
}

impl Channel {
    pub fn includes(self, other: Self) -> bool {
        match self {
            Self::Main => matches!(other, Self::Main),

            // Include all `main` examples in `release`
            Self::Release => {
                matches!(other, Self::Main | Self::Release)
            }

            // Include all `main` and `release` examples in `nightly`
            Self::Nightly => {
                matches!(other, Self::Main | Self::Release | Self::Nightly)
            }
        }
    }

    pub fn examples(self, workspace_root: impl AsRef<Path>) -> anyhow::Result<Vec<Example>> {
        // currently we only treat Python examples as runnable
        let language = Language::Python;

        let mut examples = vec![];

        let dir = workspace_root
            .as_ref()
            .join("examples")
            .join(language.examples_dir());
        if !dir.exists() {
            anyhow::bail!("Failed to find {dir:?}")
        }
        if !dir.is_dir() {
            anyhow::bail!("{dir:?} is not a directory")
        }

        let folders: std::collections::BTreeMap<String, std::fs::DirEntry> =
            std::fs::read_dir(&dir)?
                .filter_map(Result::ok)
                .map(|folder| {
                    let name = folder.file_name().to_string_lossy().to_string();
                    (name, folder)
                })
                .collect();

        for (name, folder) in folders {
            let metadata = folder.metadata()?;
            let readme_path = folder.path().join("README.md");
            if metadata.is_dir() && readme_path.exists() {
                let Some((readme, body)) = Frontmatter::load(&readme_path)? else {
                    eprintln!("{name:?}: skipped - MISSING FRONTMATTER");
                    continue;
                };

                let Some(channel) = readme.channel else {
                    eprintln!("{name:?}: skipped - missing `channel` in frontmatter");
                    continue;
                };

                if !self.includes(channel) {
                    eprintln!("{name:?}: skipped");
                    continue;
                }

                eprintln!("{name:?}: added");
                let dir = folder.path();
                examples.push(Example {
                    name,
                    title: readme.title,
                    dir,
                    description: readme.description,
                    tags: readme.tags,
                    thumbnail_url: readme.thumbnail,
                    thumbnail_dimensions: readme.thumbnail_dimensions,
                    script_args: readme.build_args,
                    readme_body: body,
                    language: Language::Python,
                });
            }
        }

        if examples.is_empty() {
            anyhow::bail!("No examples found in {dir:?}")
        }

        examples.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(examples)
    }
}

impl Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Main => "main",
            Self::Nightly => "nightly",
            Self::Release => "release",
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
            "release" => Ok(Self::Release),
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

impl Frontmatter {
    fn load(path: &Path) -> anyhow::Result<Option<(Self, String)>> {
        const START: &str = "<!--[metadata]";
        const END: &str = "-->";

        let content =
            std::fs::read_to_string(path).with_context(|| format!("loading {}", path.display()))?;

        let Some(start) = content.find(START) else {
            return Ok(None);
        };
        let start = start + START.len();

        let Some(end) = content[start..].find(END) else {
            anyhow::bail!(
                "{:?} has invalid frontmatter: missing {END:?} terminator",
                path
            );
        };
        let end = start + end;

        let frontmatter: Self = toml::from_str(content[start..end].trim()).map_err(|err| {
            #[allow(clippy::unwrap_used)]
            let p = path.parent().unwrap().file_name().unwrap();
            anyhow::anyhow!("Failed to parse TOML metadata of {p:?}: {err}")
        })?;

        Ok(Some((
            frontmatter,
            content[end + END.len()..].trim().to_owned(),
        )))
    }
}
