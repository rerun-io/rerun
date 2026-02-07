//! Codegen fbs file that are used for subsequent codegen steps.

use camino::{Utf8Path, Utf8PathBuf};

use super::autogen_warning;
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

    fn add_include_for(
        &self,
        reporter: &crate::Reporter,
        files_to_write: &mut std::collections::BTreeMap<Utf8PathBuf, String>,
        directory: &str,
    ) {
        files_to_write.insert(
            self.definition_dir.join(format!("{directory}.fbs")),
            generate_include_file_for_dir(reporter, &self.definition_dir.join(directory)),
        );
    }
}

impl CodeGenerator for FbsCodeGenerator {
    fn generate(
        &mut self,
        reporter: &crate::Reporter,
        _objects: &crate::Objects,            // Expected to be empty.
        _type_registry: &crate::TypeRegistry, // Expected to be empty.
    ) -> crate::GeneratedFiles {
        let mut files_to_write = GeneratedFiles::default();

        self.add_include_for(reporter, &mut files_to_write, "attributes");

        self.add_include_for(reporter, &mut files_to_write, "rerun/datatypes");
        self.add_include_for(reporter, &mut files_to_write, "rerun/components");
        self.add_include_for(reporter, &mut files_to_write, "rerun/archetypes");

        self.add_include_for(reporter, &mut files_to_write, "rerun/blueprint/datatypes");
        self.add_include_for(reporter, &mut files_to_write, "rerun/blueprint/components");
        self.add_include_for(reporter, &mut files_to_write, "rerun/blueprint/archetypes");
        self.add_include_for(reporter, &mut files_to_write, "rerun/blueprint/views");

        files_to_write
    }
}

fn generate_include_file_for_dir(reporter: &crate::Reporter, dir_path: &Utf8Path) -> String {
    let mut contents = format!("// {}", autogen_warning!());
    contents.push_str("\n\n");

    let read_dir = match dir_path.read_dir() {
        Ok(read_dir) => read_dir,
        Err(err) => {
            reporter.error_file(dir_path, err);
            return contents;
        }
    };

    let dir_name = dir_path.file_name().unwrap();

    let mut include_entries = Vec::new();
    for entry in read_dir {
        match entry {
            Ok(entry) => {
                let entry_path = entry.path();
                let entry_name = entry_path.file_name().unwrap();
                let entry_name = entry_name.to_str().unwrap();

                if entry_path.is_file() && entry_name.ends_with(".fbs") {
                    include_entries.push(format!("include \"./{dir_name}/{entry_name}\";\n"));
                }
            }
            Err(err) => {
                reporter.error_file(dir_path, err);
                return contents;
            }
        }
    }

    include_entries.sort();
    for include_entry in include_entries {
        contents.push_str(&include_entry);
    }

    contents
}
