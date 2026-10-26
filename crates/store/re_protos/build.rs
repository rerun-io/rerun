// Build scripts run on the host, including when the wasm Clippy config is active.
#![allow(clippy::disallowed_methods)]

use std::{fs, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH")? == "wasm32" {
        return Ok(());
    }

    // Watch the directory as well as its files to detect added and removed schemas.
    println!("cargo:rerun-if-changed=proto");
    let mut protos = fs::read_dir("proto/rerun/v1alpha1")?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    protos.retain(|path| path.extension().is_some_and(|ext| ext == "proto"));
    protos.sort();

    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?)
        .join("rerun.v1alpha1.fds.bin");
    let temporary = output.with_extension("tmp");
    let result = Command::new(protoc_bin_vendored::protoc_bin_path()?)
        .arg("--include_imports")
        .arg("--include_source_info")
        .arg("--descriptor_set_out")
        .arg(&temporary)
        .arg("-I")
        .arg("proto")
        .arg("-I")
        .arg(protoc_bin_vendored::include_path()?)
        .args(protos)
        .output()?;
    if !result.status.success() {
        return Err(format!(
            "Failed to compile protobuf descriptors: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }

    let bytes = fs::read(&temporary)?;
    // Preserve the mtime when an input was touched without changing the descriptor.
    if !output.exists() || fs::read(&output)? != bytes {
        fs::rename(temporary, output)?;
    } else {
        fs::remove_file(temporary)?;
    }
    Ok(())
}
