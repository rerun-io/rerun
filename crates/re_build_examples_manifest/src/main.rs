//! This build script generates the `examples_manifest.json` file.
//! It looks at all examples in the workspace (under `examples/python`),
//! and only includes those with `demo` set to `true` in their `README.md`
//! frontmatter.
//!
//! The URLs embedded in the `example_manifest.json` file point to a specific version.
//! This version is resolved according to the current environment:
//!
//! If the `CI` env var is set + the branch name is not `main`, then:
//! - On any `release-x.y.z` branch, the version is `version/x.y.z`
//! - On any other branch, the version is `commit/$COMMIT_SHORT_HASH`
//!
//! Otherwise, the version is `version/nightly`. This means local builds,
//! and builds on `main` point to `version/nightly`.

use std::path::Path;
use std::path::PathBuf;

use re_build_tools::Environment;

const USAGE: &str = "\
Usage: [options] [output_path]

Options:
    -h, --help       Print help
        --base-url   Where all examples are uploaded, e.g. `https://demo.rerun.io/version/nightly`.
";

fn main() -> anyhow::Result<()> {
    re_build_tools::set_output_cargo_build_instructions(false);

    let args = Args::from_env();

    let manifest = build_examples_manifest(Environment::detect(), &args)?;
    std::fs::write(args.output_path, manifest)?;

    Ok(())
}

struct Args {
    output_path: PathBuf,
    base_url: Option<String>,
}

impl Args {
    fn from_env() -> Self {
        let mut output_path = None;
        let mut base_url = None;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    println!("{USAGE}");
                    std::process::exit(1);
                }
                "--base-url" => {
                    let Some(url) = args.next() else {
                        eprintln!("Expected value after \"--base-url\"");
                        println!("\n{USAGE}");
                        std::process::exit(1);
                    };
                    base_url = Some(url);
                }
                _ if arg.starts_with('-') => {
                    eprintln!("Unknown argument: {arg:?}");
                    println!("\n{USAGE}");
                    std::process::exit(1);
                }
                _ if output_path.is_some() => {
                    eprintln!("Too many positional arguments");
                    println!("\n{USAGE}");
                    std::process::exit(1);
                }
                _ => output_path = Some(PathBuf::from(arg)),
            }
        }

        let Some(output_path) = output_path else {
            eprintln!("Missing argument \"output_path\"");
            std::process::exit(1);
        };

        Args {
            output_path,
            base_url,
        }
    }
}

fn build_examples_manifest(build_env: Environment, args: &Args) -> anyhow::Result<String> {
    let base_url = match &args.base_url {
        Some(base_url) => base_url.clone(),
        None => get_base_url(build_env)?,
    };

    let mut manifest = vec![];
    for example in examples()? {
        manifest.push(ManifestEntry::new(example, &base_url));
    }

    if manifest.is_empty() {
        anyhow::bail!("No examples found!");
    }

    Ok(serde_json::to_string_pretty(&manifest)?)
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
}

#[derive(serde::Serialize)]
struct ManifestEntry {
    name: String,
    title: String,
    description: String,
    tags: Vec<String>,
    demo_url: String,
    rrd_url: String,
    thumbnail: Thumbnail,
}

impl ManifestEntry {
    fn new(example: Example, base_url: &str) -> Self {
        let Example { name, readme } = example;
        Self {
            title: readme.title,
            description: readme.description,
            tags: readme.tags,
            demo_url: format!("{base_url}/examples/{name}/"),
            rrd_url: format!("{base_url}/examples/{name}/data.rrd"),
            thumbnail: Thumbnail {
                url: readme.thumbnail,
                width: readme.thumbnail_dimensions[0],
                height: readme.thumbnail_dimensions[1],
            },
            name,
        }
    }
}

#[derive(serde::Serialize)]
struct Thumbnail {
    url: String,
    width: u64,
    height: u64,
}

struct Example {
    name: String,
    readme: Frontmatter,
}

fn examples() -> anyhow::Result<Vec<Example>> {
    let mut examples = vec![];
    let dir = Path::new("examples/python");
    if !dir.exists() {
        anyhow::bail!("Failed to find {}", dir.display())
    }
    if !dir.is_dir() {
        anyhow::bail!("{} is not a directory", dir.display())
    }

    for folder in std::fs::read_dir(dir)? {
        let folder = folder?;
        let metadata = folder.metadata()?;
        let name = folder.file_name().to_string_lossy().to_string();
        let readme = folder.path().join("README.md");
        if metadata.is_dir() && readme.exists() {
            let readme = parse_frontmatter(readme)?;
            if let Some(readme) = readme {
                if readme.demo {
                    eprintln!("Adding example {name:?}");
                    examples.push(Example { name, readme });
                } else {
                    eprintln!("Skipping example {name:?} because 'demo' is set to 'false'");
                }
            } else {
                eprintln!("Skipping example {name:?} because it has no frontmatter");
            }
        }
    }

    if examples.is_empty() {
        anyhow::bail!("No examples found in {}", dir.display())
    }

    examples.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok(examples)
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

fn get_base_url(build_env: Environment) -> anyhow::Result<String> {
    // In the CondaBuild environment we can't trust the git_branch name -- if it exists
    // at all it's going to be the feedstock branch-name, not our Rerun branch. However
    // conda should ONLY be building released versions, so we want to version the manifest.
    let versioned_manifest = matches!(build_env, Environment::CondaBuild) || {
        let branch = re_build_tools::git_branch()?;
        if branch == "main" || !re_build_tools::is_on_ci() {
            // on `main` and local builds, use `version/nightly`
            // this will point to data uploaded by `.github/workflows/reusable_upload_web_demo.yml`
            // on every commit to the `main` branch
            return Ok("https://demo.rerun.io/version/nightly".into());
        }
        parse_release_version(&branch).is_some()
    };

    if versioned_manifest {
        let metadata = re_build_tools::cargo_metadata()?;
        let workspace_root = metadata
            .root_package()
            .ok_or_else(|| anyhow::anyhow!("failed to find workspace root"))?;

        // on `release-x.y.z` builds, use `version/{crate_version}`
        // this will point to data uploaded by `.github/workflows/reusable_build_and_publish_web.yml`
        return Ok(format!(
            "https://demo.rerun.io/version/{}",
            workspace_root.version
        ));
    }

    // any other branch that is not `main`, use `commit/{sha}`
    // this will point to data uploaded by `.github/workflows/reusable_upload_web_demo.yml`
    let sha = re_build_tools::git_commit_short_hash()?;
    Ok(format!("https://demo.rerun.io/commit/{sha}"))
}

fn parse_release_version(branch: &str) -> Option<&str> {
    // release-\d+.\d+.\d+(-alpha.\d+)?

    let version = branch.strip_prefix("release-")?;

    let (major, rest) = version.split_once('.')?;
    major.parse::<u8>().ok()?;
    let (minor, rest) = rest.split_once('.')?;
    minor.parse::<u8>().ok()?;
    let (patch, meta) = rest
        .split_once('-')
        .map_or((rest, None), |(p, m)| (p, Some(m)));
    patch.parse::<u8>().ok()?;

    if let Some(meta) = meta {
        let (kind, n) = meta.split_once('.')?;
        if kind != "alpha" && kind != "rc" {
            return None;
        }
        n.parse::<u8>().ok()?;
    }

    Some(version)
}
