use crate::CodeFormatter;

// ---

pub struct RustCodeFormatter;

impl CodeFormatter for RustCodeFormatter {
    fn format(&mut self, _reporter: &crate::Reporter, files: &mut crate::GeneratedFiles) {
        use rayon::prelude::*;

        re_tracing::profile_wait!("format_code");

        files.par_iter_mut().for_each(|(filepath, contents)| {
            *contents = if matches!(filepath.extension(), Some("rs")) {
                format_code(contents)
            } else {
                contents.clone()
            };
        });
    }
}

fn format_code(contents: &str) -> String {
    re_tracing::profile_function!();

    let mut contents = contents.replace(" :: ", "::"); // Fix `bytemuck :: Pod` -> `bytemuck::Pod`.

    // Even though we already have used `prettyplease` we also
    // need to run `cargo fmt`, since it catches some things `prettyplease` missed.
    // We need to run `cago fmt` several times because it is not idempotent;
    // see https://github.com/rust-lang/rustfmt/issues/5824
    for _ in 0..2 {
        // NOTE: We're purposefully ignoring the error here.
        //
        // In the very unlikely chance that the user doesn't have the `fmt` component installed,
        // there's still no good reason to fail the build.
        //
        // The CI will catch the unformatted file at PR time and complain appropriately anyhow.

        re_tracing::profile_scope!("rust-fmt");
        use rust_format::Formatter as _;

        // TODO(#9943): Use 2024 edition
        if let Ok(formatted) = rust_format::RustFmt::default().format_str(&contents) {
            contents = formatted;
        }
    }

    contents
}
