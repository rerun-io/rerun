/// Run `rustfmt` on some Rust code.
pub fn rustfmt_str(source: &str) -> Option<String> {
    // We need to run `cago fmt` several times because it is not idempotent;
    // see https://github.com/rust-lang/rustfmt/issues/5824
    let source = rustfmt_once(source)?;
    rustfmt_once(&source)
}

fn rustfmt_once(source: &str) -> Option<String> {
    use std::io::Write as _;
    use std::process::Stdio;

    let rust_fmt = std::env::var_os("RUSTFMT")
        .map(|s| s.display().to_string())
        .unwrap_or_else(|| String::from("rustfmt"));

    // Launch rustfmt
    let mut proc = std::process::Command::new(&rust_fmt)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--edition=2024")
        .spawn()
        .ok()?;

    // Get stdin and send our source code to it to be formatted
    let mut stdin = proc.stdin.take()?;
    stdin.write_all(source.as_bytes()).ok()?;

    drop(stdin); // Close stdin

    // Parse the results and return stdout/stderr
    let output = proc.wait_with_output().ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        // let stderr = String::from_utf8(output.stderr).ok()?;
        None
    }
}
