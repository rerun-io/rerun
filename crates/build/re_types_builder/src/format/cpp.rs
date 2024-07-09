use crate::CodeFormatter;

// ---

pub struct CppCodeFormatter;

impl CodeFormatter for CppCodeFormatter {
    fn format(&mut self, _reporter: &crate::Reporter, files: &mut crate::GeneratedFiles) {
        use rayon::prelude::*;

        re_tracing::profile_wait!("format_code");

        files.par_iter_mut().for_each(|(filepath, contents)| {
            *contents = if matches!(filepath.extension(), Some("cpp" | "hpp")) {
                format_code(contents)
            } else {
                contents.clone()
            };
        });
    }
}

fn format_code(code: &str) -> String {
    clang_format::clang_format_with_style(code, &clang_format::ClangFormatStyle::File)
        .expect("Failed to run clang-format")
}
