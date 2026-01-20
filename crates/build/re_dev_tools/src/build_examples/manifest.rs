use std::path::PathBuf;

use re_build_tools::Environment;

use super::{Channel, Example};

/// Collect examples in the repository and produce a manifest file.
///
/// The manifest file contains example metadata, such as their names
/// and links to `.rrd` files.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "manifest")]
pub struct Manifest {
    #[argh(
        positional,
        description = "output path for the manifest file (must not exist)"
    )]
    output_path: PathBuf,

    #[argh(
        option,
        description = "where examples are uploaded, e.g. `https://app.rerun.io/version/main`"
    )]
    base_url: Option<String>,

    #[argh(option, description = "include only examples in this channel")]
    channel: Channel,
}

impl Manifest {
    pub fn run(self) -> anyhow::Result<()> {
        let build_env = Environment::detect();

        let base_url = if matches!(self.channel, Channel::Nightly) {
            "https://app.rerun.io/version/nightly".to_owned()
        } else {
            match &self.base_url {
                Some(base_url) => base_url.clone(),
                None => get_base_url(build_env)?,
            }
        };

        let base_source_url = get_base_source_url(build_env)?;

        let workspace_root = re_build_tools::cargo_metadata()?.workspace_root;
        let manifest = self
            .channel
            .examples(workspace_root)?
            .into_iter()
            .filter(|example| example.include_in_manifest)
            .map(|example| ManifestEntry::new(example, &base_url, &base_source_url))
            .collect::<Vec<_>>();

        if manifest.is_empty() {
            anyhow::bail!("No examples found!");
        }

        std::fs::write(self.output_path, serde_json::to_string_pretty(&manifest)?)?;

        Ok(())
    }
}

#[derive(serde::Serialize)]
struct ManifestEntry {
    name: String,
    title: String,
    description: String,
    tags: Vec<String>,
    rrd_url: String,
    thumbnail: Thumbnail,
    source_url: String,
}

impl ManifestEntry {
    fn new(example: Example, base_url: &str, base_source_url: &str) -> Self {
        let name = example.name;
        Self {
            title: example.title,
            description: example.description,
            tags: example.tags,
            rrd_url: format!("{base_url}/examples/{name}.rrd"),
            thumbnail: Thumbnail {
                url: example.thumbnail_url,
                width: example.thumbnail_dimensions[0],
                height: example.thumbnail_dimensions[1],
            },
            source_url: format!(
                "{base_source_url}/examples/{}/{name}",
                example.language.examples_dir().to_string_lossy(),
            ),
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

fn get_base_url(build_env: Environment) -> anyhow::Result<String> {
    // In the CondaBuild environment we can't trust the git_branch name -- if it exists
    // at all it's going to be the feedstock branch-name, not our Rerun branch. However
    // conda should ONLY be building released versions, so we want to version the manifest.
    let versioned_manifest = build_env == Environment::CondaBuild || {
        let branch = re_build_tools::git_branch()?;
        if branch == "main" || build_env != Environment::RerunCI {
            // on `main` and local builds, use `version/main`
            // this will point to data uploaded by `.github/workflows/reusable_upload_examples.yml`
            // on every commit to the `main` branch
            return Ok("https://app.rerun.io/version/main".into());
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
            "https://app.rerun.io/version/{}",
            workspace_root.version
        ));
    }

    // any other branch that is not `main`, use `commit/{sha}`
    // this will point to data uploaded by `.github/workflows/reusable_upload_examples.yml`
    let sha = re_build_tools::git_commit_short_hash()?;
    Ok(format!("https://app.rerun.io/commit/{sha}"))
}

fn get_base_source_url(build_env: Environment) -> anyhow::Result<String> {
    if build_env == Environment::DeveloperInWorkspace {
        // There is a high chance the current commit isn't pushed to the remote, so we use main
        // instead.
        Ok("https://github.com/rerun-io/rerun/blob/main".to_owned())
    } else {
        let commit = re_build_tools::git_commit_short_hash()?;
        Ok(format!("https://github.com/rerun-io/rerun/tree/{commit}"))
    }
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
