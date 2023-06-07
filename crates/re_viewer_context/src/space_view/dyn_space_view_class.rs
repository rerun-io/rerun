use nohash_hasher::IntSet;
use re_data_store::EntityPropertyMap;
use re_log_types::{ComponentName, EntityPath};

use crate::{Scene, SpaceViewId, ViewerContext};

/// First element is the primary component, all others are optional.
///
/// TODO(andreas/clement): More formal definition of an archetype.
pub type ArchetypeDefinition = vec1::Vec1<ComponentName>;

re_string_interner::declare_new_type!(
    /// The unique name of a space view type.
    #[derive(serde::Deserialize, serde::Serialize)]
    pub struct SpaceViewClassName;
);

/// Defines a class of space view without any concrete types making it suitable for storage and interfacing.
///
/// Implemented by [`crate::SpaceViewClass`].
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
pub trait DynSpaceViewClass {
    /// Name of this space view class.
    ///
    /// Used for both ui display and identification.
    /// Must be unique within a viewer session.
    fn name(&self) -> SpaceViewClassName;

    /// Icon used to identify this space view class.
    fn icon(&self) -> &'static re_ui::Icon;

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi, state: &dyn SpaceViewState) -> egui::WidgetText;

    /// Called once for every new space view instance of this class.
    ///
    /// The state is *not* persisted across viewer sessions, only shared frame-to-frame.
    fn new_state(&self) -> Box<dyn SpaceViewState>;

    /// Returns a new scene for this space view class.
    ///
    /// Called both to determine the supported archetypes and
    /// to populate a scene every frame.
    fn new_scene(&self) -> Box<dyn Scene>;

    /// Optional archetype of the Space View's blueprint properties.
    ///
    /// Blueprint components that only apply to the space view itself, not to the entities it displays.
    fn blueprint_archetype(&self) -> Option<ArchetypeDefinition>;

    /// Preferred aspect ratio for the ui tiles of this space view.
    fn preferred_tile_aspect_ratio(&self, state: &dyn SpaceViewState) -> Option<f32>;

    /// Executed before the scene is populated, can be use for heuristic & state updates before populating the scene.
    ///
    /// Is only allowed to access archetypes defined by [`Self::blueprint_archetype`]
    /// Passed entity properties are individual properties without propagated values.
    fn prepare_populate(
        &self,
        ctx: &mut ViewerContext<'_>,
        state: &mut dyn SpaceViewState,
        entity_paths: &IntSet<EntityPath>,
        entity_properties: &mut EntityPropertyMap,
    );

    /// Ui shown when the user selects a space view of this class.
    ///
    /// TODO(andreas): Should this be instead implemented via a registered `data_ui` of all blueprint relevant types?
    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    );

    /// Draws the ui for this space view type and handles ui events.
    ///
    /// The scene passed in was previously created by [`Self::new_scene`] and got populated by the time it is passed.
    /// The state passed in was previously created by [`Self::new_state`] and is kept frame-to-frame.
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        scene: Box<dyn Scene>,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    );
}

/// Unserialized frame to frame state of a space view.
///
/// For any state that should be persisted, use the Blueprint!
/// This state is used for transient state, such as animation or uncommitted ui state like dragging a camera.
/// (on mouse release, the camera would be committed to the blueprint).
pub trait SpaceViewState: std::any::Any {
    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
