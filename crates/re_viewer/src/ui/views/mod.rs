// TODO(cmc): all of this stuff should simply be public, it's not this module's decision
// whether the stuff declared here is visible outside of the crate or not.
// For now we cannot do otherwise though, because we depend on some pub(crate) stuff.

// TODO: not "views" => "interactors"

pub(crate) mod view_text_entry;
pub(crate) use self::view_text_entry::{view_text_entry, SceneText, TextEntry, ViewTextEntryState};

pub(crate) mod view_tensor;
pub(crate) use self::view_tensor::{view_tensor, SceneTensor, TensorViewState};
