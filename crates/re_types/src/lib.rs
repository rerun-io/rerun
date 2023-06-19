//! The standard Rerun data types, component types, and archetypes.
//!
//! This crate contains both the IDL definitions for Rerun types (flatbuffers) as well as the code
//! generated from those using `re_types_builder`.
//!
//!
//! ### Organization
//!
//! - `definitions/` contains IDL definitions for all Rerun types (data, components, archetypes).
//! - `src/` contains the code generated for Rust.
//! - `rerun_py/rerun2/` (at the root of this workspace) contains the code generated for Python.
//!
//! While most of the code in this crate is auto-generated, some manual extensions are littered
//! throughout: look for files ending in `_ext.rs` or `_ext.py` (also see the "Extensions" section
//! of this document).
//!
//!
//! ### Build cache
//!
//! Updating either the source code of the code generator itself (`re_types_builder`) or any of the
//! .fbs files should re-trigger the code generation process the next time `re_types` is built.
//! Manual extension files will be left untouched.
//!
//! Caching is controlled by a versioning hash that is stored in `store_hash.txt`.
//! If you suspect something is wrong with the caching mechanism and that your changes aren't taken
//! into account when they should, try and remove `source_hash.txt`.
//! If that fixes the issue, you've found a bug.
//!
//!
//! ### How-to: add a new datatype/component/archetype
//!
//! Create the appropriate .fbs file in the appropriate place, and make sure it gets included in
//! some way (most likely indirectly) by `archetypes.fbs`, which is the main entrypoint for
//! codegen.
//! Generally, the easiest thing to do is to add your new type to one of the centralized manifests,
//! e.g. for a new component, include it into `components.fbs`.
//!
//! Your file should get picked up automatically by the code generator.
//! Once the code for your new component has been generated, implement whatever extensions you need
//! and make sure to tests any custom constructors you add.
//!
//!
//! ### How-to: remove an existing datatype/component/archetype
//!
//! Simply get rid of the type in question and rebuild `re_types` to trigger codegen.
//!
//! Beware though: if you remove a whole definition file re-running codegen will not remove the
//! associated generated files, you'll have to do that yourself.
//!
//!
//! ### Extensions
//!
//!
//! #### Rust
//!
//! Generated Rust code can be manually extended by adding sibling files with the `_ext.rs`
//! prefix. E.g. to extend `vec2d.rs`, create a `vec2d_ext.rs`.
//!
//! Trigger the codegen (e.g. by removing `source_hash.txt`) to generate the right `mod` clauses
//! automatically.
//!
//! The simplest way to get started is to look at any of the existing examples.
//!
//!
//! #### Python
//!
//! Generated Python code can be manually extended by adding a sibling file with the `_ext.py`
//! prefix. E.g. to extend `vec2d.py`, create a `vec2d_ext.py`.
//!
//! This sibling file needs to implement an extension class that is mixed in with the
//! auto-generated class.
//! The simplest way to get started is to look at any of the existing examples.

// ---

// TODO: gonna have to think about arrow tables formatting at some point... unless we always go
// through datacells & co in which case we're good

// TODO: whether to_arrow can fail is debatable I guess... from_arrow on the other hand can and
// will fail all the time...
// Maybe we just provide try_ helpers for both of those and call it a day?
// TODO: scatter err handling everywhere in the generated code

use std::borrow::Cow;

pub type DatatypeName = ::std::borrow::Cow<'static, str>;

/// A [`Datatype`] describes plain old data.
pub trait Datatype {
    fn name() -> DatatypeName;

    fn to_arrow_datatype() -> arrow2::datatypes::DataType;

    // TODO: takes in a bunch of values for the given DataType, and turns it into a cell
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<Cow<'a, Self>>>,
    ) -> ::re_log_types::DataCell
    where
        Self: Clone + 'a;

    // TODO: takes in a DataCell and turns it into a bunch of values for the given DataType
    fn from_arrow(cell: &::re_log_types::DataCell) -> Vec<Self>
    where
        Self: Sized;
}

pub type ComponentName = ::std::borrow::Cow<'static, str>;

pub trait Component {
    fn name() -> ComponentName;

    fn to_arrow_datatype() -> arrow2::datatypes::DataType;

    // TODO: takes in a bunch of values for the given Component, and turns it into a cell
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<Cow<'a, Self>>>,
    ) -> ::re_log_types::DataCell
    where
        Self: Clone + 'a;

    // TODO: takes in a DataCell and turns it into a bunch of values for the given Component
    fn from_arrow(cell: &::re_log_types::DataCell) -> Vec<Self>
    where
        Self: Sized;
}

pub type ArchetypeName = ::std::borrow::Cow<'static, str>;

// TODO: keep in mind: an archetype is a row, where each cell is an array of values for a single
// component.
// Should we straight up use DataRows etc in those APIs?

pub trait Archetype {
    fn name() -> ArchetypeName;

    fn required_components() -> Vec<ComponentName>;
    fn recommended_components() -> Vec<ComponentName>;
    fn optional_components() -> Vec<ComponentName>;

    fn to_arrow_datatypes() -> Vec<arrow2::datatypes::DataType>;
    // TODO: turns the archetype into a datarow
    // TODO: datarow is not possible at includes time information that we don't quite have at this
    // point
    fn to_arrow(&self) -> ::re_log_types::DataCellVec;
    // TODO: takes in a datarow, turns it back into an archetype
    // fn from_arrow(cells: &[&::re_log_types::DataCell]) -> Self
    // where
    //     Self: Sized;
}

// TODO: doc/explanations + doc-examples

pub fn to_arrow<'a, C: Component + Clone + ?Sized + 'a>(
    data: impl IntoIterator<Item = impl Into<Cow<'a, C>>>,
) -> ::re_log_types::DataCell {
    C::to_arrow(data)
}

pub fn to_arrow_opt<'a, C: Component + Clone + ?Sized + 'a>(
    data: Option<impl IntoIterator<Item = impl Into<Cow<'a, C>>>>,
) -> Option<::re_log_types::DataCell> {
    data.map(C::to_arrow)
}

// TODO: ?
// pub fn from_arrow(
//     schema: &arrow2::datatypes::Schema,
//     data: arrow2::chunk::Chunk<Box<dyn arrow2::array::Array>>,
// ) {
//     dbg!(schema);
//     dbg!(data);
// }

// ---

/// Number of decimals shown for all vector display methods.
pub const DISPLAY_PRECISION: usize = 3;

pub mod archetypes;
pub mod components;
pub mod datatypes;
