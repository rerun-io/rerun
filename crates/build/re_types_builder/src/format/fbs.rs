use crate::CodeFormatter;

pub struct FbsCodeFormatter;

impl CodeFormatter for FbsCodeFormatter {
    fn format(&mut self, _reporter: &crate::Reporter, _files: &mut crate::GeneratedFiles) {
        // We don't have formatting for fbs files yet.
    }
}
