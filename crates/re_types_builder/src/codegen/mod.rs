/// Implements the codegen pass.
pub trait CodeGenerator {
    /// Generates user-facing code from [`crate::Objects`].
    ///
    /// Returns the paths of all generated files.
    fn generate(
        &mut self,
        objs: &crate::Objects,
        arrow_registry: &crate::ArrowRegistry,
    ) -> std::collections::BTreeSet<camino::Utf8PathBuf>;
}

// ---

mod macros {
    #![allow(unused_macros)]
    macro_rules! autogen_warning {
        () => {
            format!("DO NOT EDIT! This file was auto-generated by {}.", file!())
        };
    }
    pub(crate) use autogen_warning;
}
pub(crate) use macros::autogen_warning; // Hack for declaring macros as `pub(crate)`

// ---

mod common;
use self::common::{get_documentation, StringExt};

mod cpp;
mod python;
mod rust;

pub use self::common::write_file;
pub use self::cpp::CppCodeGenerator;
pub use self::python::PythonCodeGenerator;
pub use self::rust::RustCodeGenerator;
