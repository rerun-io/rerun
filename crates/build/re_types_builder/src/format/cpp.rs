use std::io::Write as _;
use std::process::{Command, Stdio};

use anyhow::Context as _;

use crate::CodeFormatter;

// ---

pub struct CppCodeFormatter;

impl CodeFormatter for CppCodeFormatter {
    fn format(&mut self, reporter: &crate::Reporter, files: &mut crate::GeneratedFiles) {
        use rayon::prelude::*;

        re_tracing::profile_wait!("format_code");

        files.par_iter_mut().for_each(|(filepath, contents)| {
            if matches!(filepath.extension(), Some("cpp" | "hpp")) {
                match format_code(contents) {
                    Ok(formatted) => *contents = formatted,
                    Err(err) => reporter.error_file(
                        filepath,
                        re_error::format(err.context("C++ code formatting")),
                    ),
                }
            }
        });
    }
}

fn format_code(code: &str) -> anyhow::Result<String> {
    let binary = std::env::var("CLANG_FORMAT_BINARY").unwrap_or_else(|_| "clang-format".to_owned());
    let mut child = Command::new(binary)
        .arg("--style=file")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting clang-format")?;

    child
        .stdin
        .take()
        .context("accessing clang-format stdin")?
        .write_all(code.as_bytes())
        .context("sending code to clang-format")?;

    let output = child
        .wait_with_output()
        .context("waiting for clang-format")?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        anyhow::bail!(
            "clang-format failed with {}: {stdout}\n{stderr}",
            output.status
        );
    }

    String::from_utf8(output.stdout).context("reading clang-format output")
}
