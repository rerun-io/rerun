use std::path::Path;

fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/v0.14.2/building_and_distribution.html#macos
    pyo3_build_config::add_extension_module_link_args();
    println!("cargo:rerun-if-env-changed=GITHUB_REF_TYPE");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    if let Ok(ref ref_type) = std::env::var("GITHUB_REF_TYPE") {
        // We're in CI
        if ref_type == "branch" {
            // A branch build, so we're building a development version.
            let new_version = {
                let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| {
                    println!("cargo:warning=CARGO_PKG_VERSION not set!");
                    "0.0.0".to_owned()
                });
                let git_sha = std::process::Command::new("git")
                    .args(["rev-parse", "--short", "HEAD"])
                    .output()
                    .expect("Failed to run git rev-parse")
                    .stdout;
                let git_sha = String::from_utf8(git_sha).unwrap();
                toml::Value::String(format!("{pkg_version}+{}", git_sha.trim()))
            };

            println!("cargo:warning=Overriding packaged wheel version to a development version: {new_version}.");
            generate_dev_version("pyproject.toml", new_version);
        } else {
            println!("cargo:warning=Not a branch build, so not overriding packaged wheel version!");
        }
    } else {
        println!("cargo:warning=Not in CI, so not overriding packaged wheel version!");
    }
}

/// Generates a development version number for the Python package based on the Cargo package version and the current git SHA.
/// This results in a version number like `0.1.0+abcdefg`.
/// The resulting version number is written to the `pyproject.toml` file.
fn generate_dev_version(project_file: impl AsRef<Path>, new_version: toml::Value) {
    let pyproject_toml =
        std::fs::read_to_string(project_file).unwrap_or_else(|e| panic!("Failed to read {e:?}"));

    let mut pyproject_toml: toml::Value =
        toml::from_str(&pyproject_toml).expect("Failed to parse pyproject.toml");

    let project = pyproject_toml
        .get_mut("project")
        .expect("Failed to get project");

    match project.get_mut("version") {
        Some(version) => *version = new_version,
        None => {
            project
                .as_table_mut()
                .unwrap()
                .insert("version".to_owned(), new_version);
        }
    }

    let pyproject_toml = toml::to_string(&pyproject_toml).unwrap();
    std::fs::write("pyproject.toml", pyproject_toml).unwrap();
}
