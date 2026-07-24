//! Discovers and runs Rerun web tests.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context as _, bail};
use cargo_metadata::{DependencyKind, MetadataCommand};
use tokio::process::Command;

#[derive(argh::FromArgs)]
/// Discover and run workspace web tests.
struct Args {
    /// browser to pass to wasm-pack: firefox or chrome.
    #[argh(option, default = "String::from(\"firefox\")")]
    browser: String,

    /// run tests for a single package.
    #[argh(option)]
    package: Option<String>,

    /// run wasm-bindgen-test in a visible browser.
    #[argh(switch)]
    no_headless: bool,
}

struct WebTestPackage {
    name: String,
    path: PathBuf,
    redap_server: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    let packages = discover_packages(args.package.as_deref())?;
    if packages.is_empty() {
        bail!("found no web-test packages");
    }

    for package in packages {
        run_package(&args, &package).await?;
    }

    Ok(())
}

fn discover_packages(package_filter: Option<&str>) -> anyhow::Result<Vec<WebTestPackage>> {
    let metadata = MetadataCommand::new().no_deps().exec()?;

    let workspace_members = metadata.workspace_members;
    let mut packages = metadata
        .packages
        .into_iter()
        .filter(|package| workspace_members.contains(&package.id))
        .filter(|package| package_filter.is_none_or(|filter| package.name == filter))
        .filter(|package| {
            package.dependencies.iter().any(|dep| {
                dep.name == "wasm-bindgen-test" && dep.kind == DependencyKind::Development
            })
        })
        .map(|package| {
            Ok(WebTestPackage {
                name: package.name.to_string(),
                path: package
                    .manifest_path
                    .parent()
                    .context("package manifest has no parent")?
                    .to_path_buf()
                    .into_std_path_buf(),
                redap_server: package
                    .metadata
                    .pointer("/rerun/web-test/redap-server")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    if let Some(package_filter) = package_filter
        && packages.is_empty()
    {
        bail!("package {package_filter:?} is not a discovered web-test package");
    }

    packages.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
    Ok(packages)
}

async fn run_package(args: &Args, package: &WebTestPackage) -> anyhow::Result<()> {
    if args.no_headless && package.redap_server {
        // Interactive mode parses `WASM_BINDGEN_TEST_ADDRESS` as a socket address, so it
        // cannot carry the `redap_port` URL query parameter used by headless tests.
        bail!(
            "package {:?} requires a native redap server, which is only supported in headless mode",
            package.name
        );
    }

    eprintln!("Running web tests for {}", package.name);

    let server = if package.redap_server {
        Some(
            re_server::Args {
                host: "127.0.0.1".to_owned(),
                port: 0,
                ..Default::default()
            }
            .create_server_handle()
            .await?,
        )
    } else {
        None
    };

    let mut command = Command::new("wasm-pack");
    command.arg("test");
    if !args.no_headless {
        command.arg("--headless");
    }
    command.arg(format!("--{}", args.browser));
    command.arg(&package.path);

    if let Some(server) = &server {
        command.env(
            "WASM_BINDGEN_TEST_ADDRESS",
            format!(
                "http://127.0.0.1/?redap_port={}",
                server.connect_addr().port()
            ),
        );
    } else {
        command.env_remove("WASM_BINDGEN_TEST_ADDRESS");
    }

    let status = command
        .status()
        .await
        .with_context(|| format!("failed to run wasm-pack for {}", package.name))?;

    if let Some(server) = server {
        server.shutdown_and_wait().await;
    }

    if !status.success() {
        bail!("web tests failed for {}", package.name);
    }

    Ok(())
}
