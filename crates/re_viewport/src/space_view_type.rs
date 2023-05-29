use re_log_types::ComponentName;
use re_viewer_context::{SceneQuery, ViewerContext};

/// First element is the primary component, all others are optional.
///
/// TODO(andreas/clement): More formal definition of an archetype.
pub type ArchetypeDefinition = Vec<ComponentName>;

/// Defines a type of space view.
///
/// TODO: Lots of documentation
pub trait SpaceViewType {
    /// Name of this type as shown in the ui.
    fn type_display_name(&self) -> &'static str;

    /// Icon used to identify this space view type.
    fn type_icon(&self) -> &'static str;

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
pub trait SpaceViewState {}

/// A scene is a collection of scene elements.
pub struct Scene(Vec<Box<dyn SceneElement>>);

impl Scene {
    /// List of all archetypes this type of view supports.
    fn supported_archetypes(&self) -> Vec<ArchetypeDefinition> {
        self.0.iter().map(|e| e.archetype()).collect()
    }

    /// Populates the scene for a given query.
    fn populate(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
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

    // TODO(andreas): Add method for getting draw data for a re_renderer::ViewBuilder.
}
