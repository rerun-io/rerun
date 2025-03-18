use crate::Item;
use crate::ItemCollection;
use crate::ItemContext;
use re_log_types::{TimeReal, Timeline};
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
        view_id: Option<String>,
        position: Option<glam::Vec3>,
    },

    /// Selected a view.
    View { view_id: String },

    /// Selected a container.
    Container { container_id: String },
}

fn get_position(context: &Option<ItemContext>) -> Option<glam::Vec3> {
    match context {
        Some(ItemContext::TwoD { pos, .. }) => Some(*pos),
        Some(ItemContext::ThreeD { pos, .. }) => *pos,
        _ => None,
    }
}

impl CallbackSelectionItem {
    // TODO(jan): output more things, including parts of context for data results
    //            and other things not currently available here (e.g. mouse pos)
    pub fn new(item: &Item, context: &Option<ItemContext>) -> Option<Self> {
        match item {
            Item::StoreId(_) | Item::AppId(_) | Item::ComponentPath(_) | Item::DataSource(_) => {
                None
            }
            Item::View(view_id) => Some(Self::View {
                view_id: view_id.uuid().to_string(),
            }),
            Item::Container(container_id) => Some(Self::Container {
                container_id: container_id.uuid().to_string(),
            }),

            Item::DataResult(view_id, instance_path) => Some(Self::Entity {
                entity_path: instance_path.entity_path.to_string(),
                instance_id: instance_path.instance.specific_index().map(|id| id.get()),
                view_id: Some(view_id.uuid().to_string()),
                position: get_position(context),
            }),
            Item::InstancePath(instance_path) => Some(Self::Entity {
                entity_path: instance_path.entity_path.to_string(),
                instance_id: instance_path.instance.specific_index().map(|id| id.get()),
                view_id: None,
                position: get_position(context),
            }),
        }
    }
}

#[derive(Clone)]
pub struct Callbacks {
    /// Fired when the selection changes.
    ///
    /// Examples:
    /// * Clicking on an entity
    /// * Clicking on an entity instance
    /// * Clicking on or inside a view
    /// * Clicking on a container in the left panel
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
    pub fn on_selection_change(&self, items: &ItemCollection) {
        (self.on_selection_change)(
            items
                .iter()
                .filter_map(|(item, context)| CallbackSelectionItem::new(item, context))
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
