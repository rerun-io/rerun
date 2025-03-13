use crate::Item;
use crate::ItemCollection;
use crate::ItemContext;
use re_log_types::{TimeReal, Timeline};
use std::rc::Rc;

// ======================================================================
// When changing or adding callbacks, grep for the following term:
//   CALLBACK DEFINITION
//
// When changing or adding selection items, grep for the following term:
//   SELECTION ITEM DEFINITION
// ======================================================================

// SELECTION ITEM DEFINITION
/// A single item in a selection.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum CallbackSelectionItem {
    /// Selected an (entire) entity.
    ///
    /// Examples:
    /// * A full point cloud.
    /// * A mesh.
    Entity { entity_path: String },

    /// Selected an instance within an entity.
    ///
    /// Examples:
    /// * A single point in a point cloud.
    Instance {
        entity_path: String,
        instance_id: u64,
    },

    /// Selected a view.
    View { view_id: String },

    /// Selected a container.
    Container { container_id: String },
}

impl CallbackSelectionItem {
    // TODO(jan): output more things, including parts of context for data results
    //            and other things not currently available here (e.g. mouse pos)
    pub fn new(item: &Item, _context: &Option<ItemContext>) -> Option<Self> {
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
            Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => {
                if instance_path.is_all() {
                    Some(Self::Entity {
                        entity_path: instance_path.entity_path.to_string(),
                    })
                } else {
                    Some(Self::Instance {
                        entity_path: instance_path.entity_path.to_string(),
                        instance_id: instance_path.instance.get(),
                    })
                }
            }
        }
    }
}

// CALLBACK DEFINITION
#[derive(Clone)]
pub struct Callbacks {
    /// Fired when the selection changes.
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
