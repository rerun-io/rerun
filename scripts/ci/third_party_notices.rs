#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
serde_json = "1"
sha2 = "0.10"
similar = "2.7"
spdx = { version = "0.13.5", features = ["text", "detection-inline-cache"] }
toml = "0.9"

[profile.dev]
opt-level = 3
---

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde_json::Value;
use sha2::{Digest as _, Sha256};
use similar::{Algorithm, DiffOp};
use spdx::detection::{LicenseType, Store, TextData};
use spdx::{Expression, LicenseId, LicenseItem, Licensee, ParseMode};

type Result<T, E = String> = std::result::Result<T, E>;

const USAGE: &str = "usage: third_party_notices.rs <output> [--locked] [--frozen] [--offline] [--all-features] [--no-default-features] [--features F]… [--target T]… [-m <Cargo.toml>] [-c <about.toml>]";

const CONFIDENCE: f32 = 0.8;

// Licenses that only require shipping a copy of their text, not each crate's copyright line.
const SINGLE_COPY: &[&str] = &["Apache-2.0", "MPL-2.0"];

// Additions up to this many words are a filled-in copyright line and can be dropped.
// Longer ones are usually more licenses appended to the file, which must be kept.
const MAX_DROPPED_WORDS: usize = 20;

const EXPR_MODE: ParseMode = ParseMode {
    allow_deprecated: true,
    allow_imprecise_license_names: true,
    allow_slash_as_or_operator: false,
    allow_postfix_plus_on_gpl: true,
    allow_unknown: false,
};

const CONFIG_MODE: ParseMode = ParseMode {
    allow_deprecated: true,
    allow_imprecise_license_names: false,
    allow_slash_as_or_operator: false,
    allow_postfix_plus_on_gpl: false,
    allow_unknown: false,
};

const LICENSE_WORDS: &[&str] = &[
    "LICENSE",
    "LICENCE",
    "LICENSES",
    "LICENCES",
    "COPYING",
    "COPYRIGHT",
];

const IGNORED_NAME_TOKENS: &[&str] = &[
    "LICENSE",
    "LICENCE",
    "LICENSES",
    "LICENCES",
    "COPYING",
    "COPYRIGHT",
    "TXT",
    "MD",
    "MARKDOWN",
    "RST",
    "HTML",
];

const CODE_EXTENSIONS: &[&str] = &[
    "rs", "toml", "json", "yml", "yaml", "c", "h", "cc", "cpp", "hpp", "py", "js", "ts", "sh",
    "in", "lock",
];

#[derive(Default)]
struct Args {
    output: Option<PathBuf>,
    cargo: Vec<String>,
    targets: Vec<String>,
    manifest_path: Option<PathBuf>,
    config: Option<PathBuf>,
}

struct Config {
    accepted: Vec<Licensee>,
    crates: HashMap<String, CrateConfig>,
}

#[derive(Default)]
struct CrateConfig {
    accepted: Vec<Licensee>,
    clarify: Option<Clarify>,
}

struct Clarify {
    license: Expression,
    files: Vec<ClarifyFile>,
}

struct ClarifyFile {
    path: PathBuf,
    license: Option<Expression>,
    start: Option<String>,
    end: Option<String>,
    checksum: String,
}

struct LicenseFile {
    ids: Vec<LicenseId>,
    text: String,
}

struct Crate<'a> {
    package: &'a Value,
    requirements: Vec<LicenseId>,
    files: Vec<LicenseFile>,
    notices: Vec<String>,
}

#[derive(Default)]
struct Stats {
    candidates: AtomicUsize,
    by_name: AtomicUsize,
    by_content: AtomicUsize,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let started = Instant::now();
    let args = parse_args()?;
    let output = args.output.clone().ok_or(USAGE)?;
    let manifest_path = match args.manifest_path.clone() {
        Some(path) => path,
        None => std::env::current_dir()
            .map_err(|err| err.to_string())?
            .join("Cargo.toml"),
    };
    let config = load_config(args.config.as_deref(), &manifest_path)?;

    let (metadata, closure) = std::thread::scope(|s| {
        let metadata = s.spawn(|| cargo_metadata(&args, &manifest_path));
        let closure = s.spawn(|| cargo_tree(&args, &manifest_path));
        (metadata.join().unwrap(), closure.join().unwrap())
    });
    let (metadata, closure) = (metadata?, closure?);
    let resolved_at = started.elapsed();

    let packages: Vec<&Value> = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata output has no packages")?
        .iter()
        .filter(|p| {
            closure.contains(&(
                str_field(p, "name").to_owned(),
                str_field(p, "version").to_owned(),
            ))
        })
        .collect();

    let stats = Stats::default();
    let results = parallel_map(&packages, |package| process_crate(package, &config, &stats));
    let mut crates = Vec::with_capacity(results.len());
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(krate) => crates.push(krate),
            Err(err) => errors.push(err),
        }
    }
    if !errors.is_empty() {
        errors.sort();
        return Err(errors.join("\n"));
    }
    crates.sort_by(|a, b| {
        let key = |c: &Crate<'_>| {
            (
                str_field(c.package, "name").to_owned(),
                version_key(str_field(c.package, "version")),
            )
        };
        key(a).cmp(&key(b))
    });

    let markdown = render(&crates);
    std::fs::write(&output, markdown).map_err(|err| format!("{}: {err}", output.display()))?;

    eprintln!(
        "{} crates, {} candidate files ({} identified by name, {} by content); cargo took {:.2?}, total {:.2?}",
        crates.len(),
        stats.candidates.load(Ordering::Relaxed),
        stats.by_name.load(Ordering::Relaxed),
        stats.by_content.load(Ordering::Relaxed),
        resolved_at,
        started.elapsed(),
    );
    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |flag: &str| it.next().ok_or_else(|| format!("{flag} needs a value"));
        match arg.as_str() {
            "--locked" | "--frozen" | "--offline" | "--all-features" | "--no-default-features" => {
                args.cargo.push(arg);
            }
            "--features" | "-F" => {
                let features = value(&arg)?;
                args.cargo.extend(["--features".to_owned(), features]);
            }
            "--target" => args.targets.push(value(&arg)?),
            "-m" | "--manifest-path" => args.manifest_path = Some(value(&arg)?.into()),
            "-c" | "--config" => args.config = Some(value(&arg)?.into()),
            output if !output.starts_with('-') && args.output.is_none() => {
                args.output = Some(output.into())
            }
            other => return Err(format!("unknown argument '{other}'\n{USAGE}")),
        }
    }
    Ok(args)
}

fn load_config(explicit: Option<&Path>, manifest_path: &Path) -> Result<Config> {
    let path = match explicit {
        Some(path) => Some(path.to_owned()),
        None => manifest_path
            .ancestors()
            .skip(1)
            .map(|dir| dir.join("about.toml"))
            .find(|path| path.exists()),
    };
    let Some(path) = path else {
        return Ok(Config {
            accepted: Vec::new(),
            crates: HashMap::new(),
        });
    };
    let contents =
        std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let table: toml::Table = contents
        .parse()
        .map_err(|err| format!("{}: {err}", path.display()))?;

    let mut config = Config {
        accepted: Vec::new(),
        crates: HashMap::new(),
    };
    for (key, value) in &table {
        match key.as_str() {
            "accepted" => config.accepted = parse_licensees(value)?,
            "ignore-dev-dependencies" | "ignore-build-dependencies" => {
                if value.as_bool() != Some(true) {
                    return Err(format!("`{key} = false` is not supported"));
                }
            }
            _ => {
                let table = value
                    .as_table()
                    .ok_or_else(|| format!("unsupported config key `{key}`"))?;
                config
                    .crates
                    .insert(key.clone(), parse_crate_config(key, table)?);
            }
        }
    }
    Ok(config)
}

fn parse_licensees(value: &toml::Value) -> Result<Vec<Licensee>> {
    value
        .as_array()
        .ok_or("`accepted` must be an array")?
        .iter()
        .map(|v| {
            let s = v.as_str().ok_or("`accepted` entries must be strings")?;
            Licensee::parse_mode(s, CONFIG_MODE)
                .map_err(|err| format!("invalid licensee `{s}`: {err}"))
        })
        .collect()
}

fn parse_crate_config(name: &str, table: &toml::Table) -> Result<CrateConfig> {
    let mut config = CrateConfig::default();
    for (key, value) in table {
        match key.as_str() {
            "accepted" => config.accepted = parse_licensees(value)?,
            "clarify" => {
                let clarify = value
                    .as_table()
                    .ok_or_else(|| format!("{name}.clarify must be a table"))?;
                let license =
                    parse_config_expr(clarify.get("license"), &format!("{name}.clarify.license"))?;
                let mut files = Vec::new();
                for (k, v) in clarify {
                    match k.as_str() {
                        "license" => {}
                        "files" => {
                            for file in v
                                .as_array()
                                .ok_or_else(|| format!("{name}.clarify.files must be an array"))?
                            {
                                files.push(parse_clarify_file(name, file)?);
                            }
                        }
                        other => return Err(format!("unsupported key `{name}.clarify.{other}`")),
                    }
                }
                config.clarify = Some(Clarify { license, files });
            }
            other => return Err(format!("unsupported key `{name}.{other}`")),
        }
    }
    Ok(config)
}

fn parse_clarify_file(name: &str, value: &toml::Value) -> Result<ClarifyFile> {
    let table = value
        .as_table()
        .ok_or_else(|| format!("{name}.clarify.files entries must be tables"))?;
    let string = |key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    Ok(ClarifyFile {
        path: string("path")
            .ok_or_else(|| format!("{name}.clarify.files entry is missing `path`"))?
            .into(),
        license: match table.get("license") {
            Some(v) => Some(parse_config_expr(
                Some(v),
                &format!("{name}.clarify.files.license"),
            )?),
            None => None,
        },
        start: string("start"),
        end: string("end"),
        checksum: string("checksum")
            .ok_or_else(|| format!("{name}.clarify.files entry is missing `checksum`"))?,
    })
}

fn parse_config_expr(value: Option<&toml::Value>, what: &str) -> Result<Expression> {
    let s = value
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{what} must be a string"))?;
    Expression::parse_mode(s, CONFIG_MODE).map_err(|err| format!("{what}: {err}"))
}

fn cargo(args: &[&str], extra: &[String]) -> Result<String> {
    let output = Command::new("cargo")
        .args(args)
        .args(extra)
        .output()
        .map_err(|err| format!("failed to run cargo: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo {}` failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|err| err.to_string())
}

fn cargo_metadata(args: &Args, manifest_path: &Path) -> Result<Value> {
    let manifest_path = manifest_path.to_str().ok_or("manifest path is not utf-8")?;
    let stdout = cargo(
        &[
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            manifest_path,
        ],
        &args.cargo,
    )?;
    serde_json::from_str(&stdout).map_err(|err| err.to_string())
}

fn cargo_tree(args: &Args, manifest_path: &Path) -> Result<HashSet<(String, String)>> {
    let manifest_path = manifest_path.to_str().ok_or("manifest path is not utf-8")?;
    let mut tree_args = vec![
        "tree",
        "--edges",
        "normal",
        "--prefix",
        "none",
        "--format",
        "{p}",
        "--manifest-path",
        manifest_path,
    ];
    if args.targets.is_empty() {
        tree_args.extend(["--target", "all"]);
    }
    for target in &args.targets {
        tree_args.extend(["--target", target.as_str()]);
    }
    let stdout = cargo(&tree_args, &args.cargo)?;
    Ok(stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let version = parts.next()?.strip_prefix('v')?;
            Some((name.to_owned(), version.to_owned()))
        })
        .collect())
}

fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let next = AtomicUsize::new(0);
    let mut results: Vec<(usize, R)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        out.push((i, f(item)));
                    }
                    out
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
}

fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn version_key(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+'])
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

fn expr_ids(expression: &Expression) -> Vec<LicenseId> {
    let mut ids: Vec<_> = expression
        .requirements()
        .filter_map(|r| r.req.license.id())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn process_crate<'a>(package: &'a Value, config: &Config, stats: &Stats) -> Result<Crate<'a>> {
    let name = str_field(package, "name");
    let version = str_field(package, "version");
    let label = format!("{name} {version}");
    let root = Path::new(str_field(package, "manifest_path"))
        .parent()
        .ok_or_else(|| format!("{label}: manifest path has no parent"))?;
    let crate_config = config.crates.get(name);

    let clarify = crate_config.and_then(|c| c.clarify.as_ref());
    let expression = match (clarify, package["license"].as_str()) {
        (Some(clarify), _) => clarify.license.clone(),
        (None, Some(license)) => {
            Expression::parse_mode(&license.replace('/', " OR "), EXPR_MODE)
                .map_err(|err| format!("{label}: invalid license expression `{license}`: {err}"))?
        }
        (None, None) => {
            license_file_expression(root, package).map_err(|err| format!("{label}: {err}"))?
        }
    };

    let accepted: Vec<&Licensee> = std::iter::chain(
        &config.accepted,
        crate_config.into_iter().flat_map(|c| c.accepted.iter()),
    )
    .collect();
    if let Err(failed) =
        expression.evaluate_with_failures(|req| accepted.iter().any(|l| l.satisfies(req)))
    {
        let failed: Vec<_> = failed.iter().map(|r| r.req.to_string()).collect();
        return Err(format!(
            "{label}: `{expression}` is not accepted (rejected: {})",
            failed.join(", ")
        ));
    }
    let requirements = expression
        .minimized_requirements(accepted.iter().copied())
        .map_err(|err| format!("{label}: {err}"))?;
    let requirements: Vec<LicenseId> = requirements
        .into_iter()
        .filter_map(|req| match req.license {
            LicenseItem::Spdx { id, .. } => Some(id),
            LicenseItem::Other(other) => {
                eprintln!("warning: {label}: no text for `{other}`");
                None
            }
        })
        .collect();

    let mut paths = Vec::new();
    walk(root, &mut paths);
    let notices = read_notices(root, &paths);
    let files = match clarify {
        Some(clarify) => clarified_files(root, clarify).map_err(|err| format!("{label}: {err}"))?,
        None => scan_license_files(paths, &expr_ids(&expression), &requirements, stats),
    };

    Ok(Crate {
        package,
        requirements,
        files,
        notices,
    })
}

fn license_file_expression(root: &Path, package: &Value) -> Result<Expression> {
    let license_file = package["license_file"]
        .as_str()
        .ok_or("neither `license` nor `license-file` is set")?;
    let text = std::fs::read_to_string(root.join(license_file))
        .map_err(|err| format!("{license_file}: {err}"))?;
    let id = classify_text(&text)
        .ok_or_else(|| format!("could not identify the license in `{license_file}`"))?;
    Expression::parse(id.name).map_err(|err| err.to_string())
}

fn clarified_files(root: &Path, clarify: &Clarify) -> Result<Vec<LicenseFile>> {
    clarify
        .files
        .iter()
        .map(|file| {
            let path = root.join(&file.path);
            let contents = std::fs::read_to_string(&path)
                .map_err(|err| format!("{}: {err}", path.display()))?;
            let start = match &file.start {
                Some(start) => contents
                    .find(start.as_str())
                    .ok_or_else(|| format!("`{start}` not found in {}", path.display()))?,
                None => 0,
            };
            let end = match &file.end {
                Some(end) => {
                    contents[start..]
                        .find(end.as_str())
                        .ok_or_else(|| format!("`{end}` not found in {}", path.display()))?
                        + start
                        + end.len()
                }
                None => contents.len(),
            };
            let text = &contents[start..end];
            let checksum = format!("{:x}", Sha256::digest(text.as_bytes()));
            if checksum != file.checksum {
                return Err(format!(
                    "checksum mismatch for {}: expected {}, got {checksum}",
                    path.display(),
                    file.checksum
                ));
            }
            Ok(LicenseFile {
                ids: expr_ids(file.license.as_ref().unwrap_or(&clarify.license)),
                text: text.to_owned(),
            })
        })
        .collect()
}

fn read_notices(root: &Path, paths: &[PathBuf]) -> Vec<String> {
    let mut notices: Vec<&PathBuf> = paths
        .iter()
        .filter(|path| path.parent() == Some(root))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.to_ascii_uppercase().starts_with("NOTICE"))
        })
        .collect();
    notices.sort();
    notices
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| text.trim().to_owned())
        .collect()
}

fn scan_license_files(
    paths: Vec<PathBuf>,
    declared: &[LicenseId],
    required: &[LicenseId],
    stats: &Stats,
) -> Vec<LicenseFile> {
    let mut files = Vec::new();
    let mut others = Vec::new();
    for path in paths {
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(by_name) = candidate(file_name, declared) else {
            others.push(path);
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        stats.candidates.fetch_add(1, Ordering::Relaxed);

        let id = match by_name {
            Some(id) => {
                stats.by_name.fetch_add(1, Ordering::Relaxed);
                Some(id)
            }
            None => {
                stats.by_content.fetch_add(1, Ordering::Relaxed);
                classify_text(&text)
            }
        };
        if let Some(id) = id.filter(|id| required.contains(id)) {
            files.push(LicenseFile {
                ids: vec![id],
                text,
            });
        }
    }

    let missing: Vec<LicenseId> = required
        .iter()
        .copied()
        .filter(|id| !files.iter().any(|f| f.ids.contains(id)))
        .collect();
    if missing.is_empty() {
        return files;
    }
    for path in others {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "" | "txt" | "md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        stats.by_content.fetch_add(1, Ordering::Relaxed);
        if let Some(id) = classify_text(&text).filter(|id| missing.contains(id)) {
            files.push(LicenseFile {
                ids: vec![id],
                text,
            });
        }
    }
    files
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            if name != "target" {
                walk(&path, out);
            }
        } else if metadata.is_file() {
            out.push(path);
        }
    }
}

fn candidate(file_name: &str, declared: &[LicenseId]) -> Option<Option<LicenseId>> {
    let upper = file_name.to_ascii_uppercase();
    let tokens: Vec<&str> = upper
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let extension = Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if CODE_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }

    let has_license_word = tokens
        .iter()
        .any(|t| LICENSE_WORDS.contains(t) || t.ends_with("LICENSE") || t.ends_with("LICENCE"));
    let rest: String = tokens
        .iter()
        .filter(|t| !IGNORED_NAME_TOKENS.contains(t))
        .copied()
        .collect();
    let by_name = match_declared(&rest, declared);

    if has_license_word {
        Some(by_name)
    } else if by_name.is_some() && matches!(extension.as_str(), "" | "txt" | "md") {
        Some(by_name)
    } else {
        None
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn match_declared(rest: &str, declared: &[LicenseId]) -> Option<LicenseId> {
    if rest.is_empty() {
        return None;
    }
    let unique = |ids: Vec<LicenseId>| if ids.len() == 1 { Some(ids[0]) } else { None };

    let exact: Vec<_> = declared
        .iter()
        .copied()
        .filter(|id| normalize(id.name) == rest)
        .collect();
    if !exact.is_empty() {
        return unique(exact);
    }
    let fuzzy: Vec<_> = declared
        .iter()
        .copied()
        .filter(|id| {
            let full = normalize(id.name);
            let base = normalize(id.name.split('-').next().unwrap_or_default());
            let version_suffix = rest
                .strip_prefix(base.as_str())
                .is_some_and(|suffix| suffix.chars().all(|c| c.is_ascii_digit()));
            full.starts_with(rest) || version_suffix
        })
        .collect();
    unique(fuzzy)
}

fn store() -> &'static Store {
    static STORE: OnceLock<Store> = OnceLock::new();
    STORE.get_or_init(|| Store::load_inline().expect("failed to load the SPDX license store"))
}

fn classify_text(text: &str) -> Option<LicenseId> {
    let found = store().analyze(&TextData::new(text));
    if found.score >= CONFIDENCE && matches!(found.license_type, LicenseType::Original) {
        spdx::license_id(found.name)
    } else {
        None
    }
}

fn render(crates: &[Crate<'_>]) -> String {
    let mut texts: BTreeMap<(&str, &str), (LicenseId, Vec<&Value>)> = BTreeMap::new();
    for krate in crates {
        for &id in &krate.requirements {
            let mut found = false;
            for file in krate.files.iter().filter(|f| f.ids.contains(&id)) {
                found = true;
                texts
                    .entry((id.name, &file.text))
                    .or_insert_with(|| (id, Vec::new()))
                    .1
                    .push(krate.package);
            }
            if !found {
                texts
                    .entry((id.name, id.text()))
                    .or_insert_with(|| (id, Vec::new()))
                    .1
                    .push(krate.package);
            }
        }
    }

    let mut markdown = String::from("# Third-party licenses\n\n");
    let mut single_copy: BTreeMap<&str, Vec<(LicenseId, &str, &[&Value])>> = BTreeMap::new();
    for (&(name, text), (id, used_by)) in &texts {
        if SINGLE_COPY.contains(&name) {
            single_copy
                .entry(name)
                .or_default()
                .push((*id, text, used_by));
        } else {
            section(&mut markdown, id.full_name, &users(used_by), text);
        }
    }
    for group in single_copy.values() {
        let mut most_used = &group[0];
        for entry in group {
            if entry.2.len() > most_used.2.len() {
                most_used = entry;
            }
        }
        let all: Vec<&Value> = group
            .iter()
            .flat_map(|(_, _, used_by)| used_by.iter().copied())
            .collect();
        section(
            &mut markdown,
            most_used.0.full_name,
            &users(&all),
            most_used.1,
        );
        for (id, text, used_by) in group {
            let added = additions(most_used.1, text);
            if added.split_whitespace().count() > MAX_DROPPED_WORDS {
                section(
                    &mut markdown,
                    &format!("{}: additions", id.full_name),
                    &users(used_by),
                    &added,
                );
            }
        }
    }

    let mut notices: Vec<(&str, Vec<String>)> = Vec::new();
    let mut notice_index: HashMap<&str, usize> = HashMap::new();
    for krate in crates {
        for text in &krate.notices {
            let index = *notice_index.entry(text).or_insert_with(|| {
                notices.push((text, Vec::new()));
                notices.len() - 1
            });
            notices[index].1.push(format!(
                "{} {}",
                str_field(krate.package, "name"),
                str_field(krate.package, "version")
            ));
        }
    }
    for (text, used_by) in &notices {
        section(&mut markdown, "NOTICE", used_by, text);
    }

    markdown
}

fn section(markdown: &mut String, heading: &str, used_by: &[String], text: &str) {
    markdown.push_str(&format!("## {heading}\n\nUsed by:\n"));
    for user in used_by {
        markdown.push_str(&format!("- {user}\n"));
    }
    markdown.push_str(&format!("\n```\n{text}\n```\n\n"));
}

/// The crates using these licenses, leaving out our own workspace crates.
///
/// Our crates are kept when they are the only users, since then the text is something they bundle (like a font).
fn users(packages: &[&Value]) -> Vec<String> {
    let third_party: Vec<&Value> = packages
        .iter()
        .copied()
        .filter(|p| !p["source"].is_null())
        .collect();
    let chosen = if third_party.is_empty() {
        packages
    } else {
        &third_party
    };
    chosen
        .iter()
        .map(|p| format!("{} {}", str_field(p, "name"), str_field(p, "version")))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The parts of `text` that `common` does not have, with their original formatting.
fn additions(common: &str, text: &str) -> String {
    let spans = word_spans(text);
    let new: Vec<&str> = spans
        .iter()
        .map(|&(start, end)| &text[start..end])
        .collect();
    let old: Vec<&str> = common.split_whitespace().collect();
    similar::capture_diff_slices(Algorithm::Patience, &old, &new)
        .into_iter()
        .filter_map(|op| match op {
            DiffOp::Insert {
                new_index, new_len, ..
            }
            | DiffOp::Replace {
                new_index, new_len, ..
            } => Some(&text[spans[new_index].0..spans[new_index + new_len - 1].1]),
            DiffOp::Equal { .. } | DiffOp::Delete { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n…\n\n")
}

fn word_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                spans.push((s, i));
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        spans.push((s, text.len()));
    }
    spans
}
