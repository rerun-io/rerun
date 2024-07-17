//! Codegen fbs file that are used for subsequent codegen steps.

use camino::Utf8PathBuf;

use crate::{CodeGenerator, GeneratedFiles};

pub struct FbsCodeGenerator {
    definition_dir: Utf8PathBuf,
}

impl FbsCodeGenerator {
    pub fn new(definition_dir: impl Into<Utf8PathBuf>) -> Self {
        Self {
            definition_dir: definition_dir.into(),
        }
    }
}

impl CodeGenerator for FbsCodeGenerator {
    fn generate(
        &mut self,
        reporter: &crate::Reporter,
        _objects: &crate::Objects,              // Expected to be empty.
        _arrow_registry: &crate::ArrowRegistry, // Expected to be empty.
    ) -> crate::GeneratedFiles {
        let mut files_to_write = GeneratedFiles::default();

        files_to_write
    }
}
