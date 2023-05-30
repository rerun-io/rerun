use crate::{SceneQuery, ViewerContext};
use re_log_types::ComponentName;

/// First element is the primary component, all others are optional.
///
/// TODO(andreas/clement): More formal definition of an archetype.
pub type ArchetypeDefinition = Vec<ComponentName>;

re_string_interner::declare_new_type!(
    /// The unique name of a space view type.
    #[derive(serde::Deserialize, serde::Serialize)]
    pub struct SpaceViewClassName;
);

/// Defines a type of space view.
///
/// TODO: Lots of documentation.
pub trait SpaceViewClass {
    /// Name of this space view type.
    ///
    /// Used for both ui display and identification.
    /// Must be unique within a viewer session.
    fn type_name(&self) -> SpaceViewClassName;

    /// Icon used to identify this space view type.
    fn type_icon(&self) -> &'static re_ui::Icon;

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText;

    /// Returns a new scene for this space view type.
    ///
    /// Called both to determine the supported archetypes and
    /// to populate a scene every frame.
    fn new_scene(&self) -> Scene;

    /// Called once for every new space view instance of this type.
    ///
    /// The state is *not* persisted across viewer sessions, only shared frame-to-frame.
    fn new_state(&self) -> Box<dyn SpaceViewState>;

    /// Ui shown when the user selects a space view of this type.
    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
    );

    /// Draws the ui for this space view type and handles ui events.
    ///
    /// The scene passed in was previously created by [`Self::new_scene`] and got populated.
    /// The state passed in was previously created by [`Self::new_state`] and is kept frame-to-frame.
    ///
    /// TODO(andreas): This is called after `re_renderer` driven content has been passed to the ui.
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        scene: Scene,
    );
}

/// State of a space view.
pub trait SpaceViewState: std::any::Any {
    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// A scene is a collection of scene elements.
pub struct Scene(pub Vec<Box<dyn SceneElement>>); // TODO: use tinyvec

impl Scene {
    /// List of all archetypes this type of view supports.
    pub fn supported_archetypes(&self) -> Vec<ArchetypeDefinition> {
        self.0.iter().map(|e| e.archetype()).collect()
    }

    /// Populates the scene for a given query.
    pub fn populate(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        for element in &mut self.0 {
            element.populate(ctx, query);
        }
    }
}

pub trait SceneElement {
    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    fn populate(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>);

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    // TODO(andreas): Add method for getting draw data for a re_renderer::ViewBuilder.
}
