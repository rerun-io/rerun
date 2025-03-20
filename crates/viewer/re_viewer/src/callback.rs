use re_log_types::{TimeReal, Timeline};
use re_viewer_context::ContainerId;
use re_viewer_context::Item;
use re_viewer_context::ItemCollection;
use re_viewer_context::ItemContext;
use re_viewer_context::ViewId;
use re_viewport_blueprint::ViewportBlueprint;
use std::rc::Rc;

/// A single item in a selection.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum CallbackSelectionItem {
    /// Selected an entity, or an instance of an entity.
    ///
    /// If the entity was selected within a view, then this also
    /// includes the `view_id`.
    ///
    /// If the entity was selected within a 2D or 3D space view,
    /// then this also includes the position.
    Entity {
        entity_path: String,
        instance_id: Option<u64>,
        view_name: Option<String>,
        position: Option<glam::Vec3>,
    },

    /// Selected a view.
    View { view_id: String, view_name: String },

    /// Selected a container.
    Container {
        container_id: String,
        container_name: String,
    },
}

fn get_position(context: &Option<ItemContext>) -> Option<glam::Vec3> {
    match context {
        Some(ItemContext::TwoD { pos, .. }) => Some(*pos),
        Some(ItemContext::ThreeD { pos, .. }) => *pos,
        _ => None,
    }
}

fn get_view_name(blueprint: &ViewportBlueprint, view_id: &ViewId) -> Option<String> {
    blueprint
        .view(view_id)
        .map(|view| view.display_name_or_default().as_ref().to_owned())
}

fn get_container_name(blueprint: &ViewportBlueprint, container_id: &ContainerId) -> Option<String> {
    blueprint
        .container(container_id)
        .map(|container| container.display_name_or_default().as_ref().to_owned())
}

impl CallbackSelectionItem {
    // TODO(jan): output more things, including parts of context for data results
    //            and other things not currently available here (e.g. mouse pos)
    pub fn new(
        item: &Item,
        context: &Option<ItemContext>,
        blueprint: &ViewportBlueprint,
    ) -> Option<Self> {
        match item {
            Item::StoreId(_) | Item::AppId(_) | Item::ComponentPath(_) | Item::DataSource(_) => {
                None
            }
            Item::View(view_id) => Some(Self::View {
                view_id: view_id.uuid().to_string(),
                view_name: if let Some(name) = get_view_name(blueprint, view_id) {
                    name
                } else {
                    re_log::debug!("failed to get view name for view id {view_id}");
                    return None;
                },
            }),
            Item::Container(container_id) => Some(Self::Container {
                container_id: container_id.uuid().to_string(),
                container_name: if let Some(name) = get_container_name(blueprint, container_id) {
                    name
                } else {
                    re_log::debug!("failed to get container name for container id {container_id}");
                    return None;
                },
            }),

            Item::DataResult(view_id, instance_path) => Some(Self::Entity {
                entity_path: instance_path.entity_path.to_string(),
                instance_id: instance_path.instance.specific_index().map(|id| id.get()),
                view_name: get_view_name(blueprint, view_id),
                position: get_position(context),
            }),
            Item::InstancePath(instance_path) => Some(Self::Entity {
                entity_path: instance_path.entity_path.to_string(),
                instance_id: instance_path.instance.specific_index().map(|id| id.get()),
                view_name: None,
                position: get_position(context),
            }),
        }
    }
}

#[derive(Clone)]
pub struct Callbacks {
    /// Fired when the selection changes.
    ///
    /// This event is fired each time any part of the event payload changes,
    /// this includes for example clicking on different parts of the same
    /// entity in a 2D or 3D view.
    pub on_selection_change: Rc<dyn Fn(Vec<CallbackSelectionItem>)>,

    /// Fired when a different timeline is selected.
    pub on_timeline_change: Rc<dyn Fn(Timeline, TimeReal)>,

    /// Fired when the timepoint changes.
    ///
    /// Does not fire when `on_seek` is called.
    pub on_time_update: Rc<dyn Fn(TimeReal)>,

    /// Fired when the timeline is paused.
    pub on_pause: Rc<dyn Fn()>,

    /// Fired when the timeline is played.
    pub on_play: Rc<dyn Fn()>,
}

impl Callbacks {
    pub fn on_selection_change(&self, items: &ItemCollection, blueprint: &ViewportBlueprint) {
        (self.on_selection_change)(
            items
                .iter()
                .filter_map(|(item, context)| CallbackSelectionItem::new(item, context, blueprint))
                .collect(),
        );
    }

    pub fn on_timeline_change(&self, timeline: Timeline, time: TimeReal) {
        (self.on_timeline_change)(timeline, time);
    }

    pub fn on_time_update(&self, time: TimeReal) {
        (self.on_time_update)(time);
    }

    pub fn on_pause(&self) {
        (self.on_pause)();
    }

    pub fn on_play(&self) {
        (self.on_play)();
    }
}
