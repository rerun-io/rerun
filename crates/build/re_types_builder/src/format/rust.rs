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

    let contents = contents.replace(" :: ", "::"); // Fix `bytemuck :: Pod` -> `bytemuck::Pod`.

    // Even though we already have used `prettyplease` we also
    // need to run `cargo fmt`, since it catches some things `prettyplease` missed.

    if let Some(formatted) = re_build_tools::rustfmt_str(&contents) {
        // NOTE: We're purposefully ignoring the error here.
        //
        // In the very unlikely chance that the user doesn't have the `fmt` component installed,
        // there's still no good reason to fail the build.
        //
        // The CI will catch the unformatted file at PR time and complain appropriately anyhow.
        formatted
    } else {
        contents
    }
}
