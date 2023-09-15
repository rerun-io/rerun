use std::path::Path;

type AnyError = Box<dyn std::error::Error + Send + Sync + 'static>;
type Result<T, E = AnyError> = std::result::Result<T, E>;

#[derive(Debug)]
struct Error(String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
macro_rules! error {
    ($lit:literal) => (Error($lit.to_owned()));
    ($($tt:tt)*) => (Error(format!($($tt)*)));
}
macro_rules! bail {
    ($lit:literal) => (return Err(error!($lit)));
    ($($tt:tt)*) => (return Err(error!($($tt)*).into()));
}

#[derive(serde::Deserialize)]
struct Frontmatter {
    title: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
    thumbnail: Option<String>,
    thumbnail_dimensions: Option<[u64; 2]>,
    build_args: Option<Vec<String>>,
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

#[derive(serde::Serialize)]
struct Thumbnail {
    url: String,
    width: u64,
    height: u64,
}

impl TryFrom<Example> for ManifestEntry {
    type Error = Error;
    fn try_from(example: Example) -> Result<Self, Self::Error> {
        macro_rules! get {
            ($e:ident, $f:ident) => {
                match $e.readme.$f {
                    Some(value) => value,
                    None => bail!("{:?} is missing field {:?}", $e.name, stringify!($f)),
                }
            };
        }

        let base_url = std::env::var("EXAMPLES_MANIFEST_BASE_URL")
            .unwrap_or_else(|_e| "https://demo.rerun.io/version/nightly".into());

        let thumbnail_dimensions = get!(example, thumbnail_dimensions);

        Ok(Self {
            title: get!(example, title),
            description: get!(example, description),
            tags: get!(example, tags),
            demo_url: format!("{base_url}/examples/arkit_scenes/"),
            rrd_url: format!("{base_url}/examples/arkit_scenes/data.rrd"),
            thumbnail: Thumbnail {
                url: get!(example, thumbnail),
                width: thumbnail_dimensions[0],
                height: thumbnail_dimensions[1],
            },
            name: example.name,
        })
    }
}

struct Example {
    name: String,
    readme: Frontmatter,
}

fn examples() -> Result<Vec<Example>> {
    let mut examples = vec![];
    for folder in std::fs::read_dir("../../examples/python")? {
        let folder = folder?;
        let metadata = folder.metadata()?;
        let name = folder.file_name().to_string_lossy().to_string();
        let readme = folder.path().join("README.md");
        if metadata.is_dir() && readme.exists() {
            let readme = parse_frontmatter(readme)?;
            let Some(readme) = readme else { continue };
            if readme.build_args.is_none() {
                continue;
            }
            examples.push(Example { name, readme });
        }
    }
    Ok(examples)
}

fn parse_frontmatter<P: AsRef<Path>>(path: P) -> Result<Option<Frontmatter>> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)?;
    let Some(content) = content.strip_prefix("---\n") else {
        return Ok(None);
    };
    let Some(end) = content.find("---") else {
        bail!("{:?} has invalid frontmatter", path);
    };
    Ok(Some(serde_yaml::from_str(&content[..end]).map_err(
        |e| {
            error!(
                "failed to read {:?}: {e}",
                path.parent().unwrap().file_name().unwrap()
            )
        },
    )?))
}

const MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/examples_manifest.json");

fn write_examples_manifest() -> Result<()> {
    let mut manifest = vec![];
    for example in examples()? {
        manifest.push(ManifestEntry::try_from(example)?);
    }
    re_build_tools::write_file_if_necessary(
        MANIFEST_PATH,
        serde_json::to_string_pretty(&manifest)?.as_bytes(),
    )?;
    Ok(())
}

fn write_examples_manifest_if_necessary() {
    if !re_build_tools::is_tracked_env_var_set("IS_IN_RERUN_WORKSPACE")
        || re_build_tools::is_tracked_env_var_set("RERUN_IS_PUBLISHING")
    {
        return;
    }
    re_build_tools::rerun_if_changed_or_doesnt_exist(MANIFEST_PATH);

    if let Err(e) = write_examples_manifest() {
        panic!("{e}");
    }
}

fn main() {
    re_build_tools::rebuild_if_crate_changed("re_viewer");
    re_build_tools::export_env_vars();

    write_examples_manifest_if_necessary();
}
