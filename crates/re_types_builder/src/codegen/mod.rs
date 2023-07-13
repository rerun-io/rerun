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

pub const AUTOGEN_WARNING: &str =
    "NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.";

// ---

mod common;
use self::common::{get_documentation, StringExt};

mod cpp;
mod python;
mod rust;

pub use self::cpp::CppCodeGenerator;
pub use self::python::PythonCodeGenerator;
pub use self::rust::RustCodeGenerator;
