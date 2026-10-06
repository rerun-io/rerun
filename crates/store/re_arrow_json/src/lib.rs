//! Rerun data as JSON, and back: [`json_from_store`] reads a store, and [`chunks_from_json`] writes
//! one. Used to read and write blueprints.
//!
//! A blueprint's JSON mirrors the blueprint store row for row:
//!
//! ```json
//! {
//!   "/viewport": { "ViewportBlueprint": { "root_container": "…", "auto_views": false } },
//!   "/view/…": { "ViewBlueprint": { "class_identifier": "3D", "space_origin": "/world" } }
//! }
//! ```
//!
//! * One entry per blueprint entity path.
//! * One key per archetype on that entity, by its short name, or by its full name when another
//!   known archetype shares that short name. A component logged without an archetype sits
//!   directly on the entity, keyed by its component identifier.
//! * One key per component of that archetype, by its field name.
//!
//! Component values are encoded from their Arrow datatype by `arrow-json`, except for the few
//! with a friendlier form (UUIDs, TUIDs, colors, enums, …), all listed in `special.rs`. Unions, which
//! `arrow-json` cannot encode, are written as `arrow_union.rs` describes. A batch of one instance
//! is written as that instance rather than as a one-element array, since most blueprint
//! components are mono-components.
//!
//! A cleared (empty) component is left out, as is an entity with nothing but cleared components,
//! since the viewer treats both as absent. So are `Null`-typed components, such as the indicators
//! older blueprints carry, since they hold no value.

mod archetype_key;
mod arrow_union;
mod from_json;
mod special;
mod to_json;

#[cfg(test)]
mod test_util;

pub use self::from_json::{ChunksFromJsonError, ValueFromJsonError, chunks_from_json};
pub use self::to_json::json_from_store;
