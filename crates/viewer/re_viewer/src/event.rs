//! Viewer event definitions.
//!
//! A callback may be registered to the Viewer via [`crate::StartupOptions::on_event`]
//! which will receive instances of [`ViewerEvent`].

// NOTE: Any changes to the type definitions in this file must be replicated in:
// - rerun_js/web-viewer/index.ts (ViewerEvent)
// - rerun_py/rerun_sdk/rerun/event.py (ViewerEvent)

use std::rc::Rc;

use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, StoreId};
use re_log_types::{TimeReal, Timeline, TimelineName};
use re_viewer_context::Item;
use re_viewer_context::ItemContext;
use re_viewer_context::ViewId;
use re_viewer_context::{ContainerId, ItemCollection};
use re_viewport_blueprint::ViewportBlueprint;

/// An event produced in the Viewer.
///
/// See [`ViewerEventKind`] for information about specific events.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewerEvent {
    pub application_id: ApplicationId,

    #[serde(with = "serde::recording_id")]
    pub recording_id: StoreId,

    #[serde(flatten)]
    pub kind: ViewerEventKind,
}

impl ViewerEvent {
    #[inline]
    fn from_db_and_kind(db: &EntityDb, kind: ViewerEventKind) -> Option<Self> {
        Some(Self {
            application_id: db.app_id()?.clone(),
            recording_id: db.store_id(),
            kind,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ViewerEventKind {
    /// Fired when the timeline starts playing.
    Play,

    /// Fired when the timeline stops playing.
    Pause,

    /// Fired when the timepoint changes.
    ///
    /// Does not fire when `on_seek` is called.
    TimeUpdate {
        #[serde(with = "serde::time_real")]
        time: TimeReal,
    },

    /// Fired when a different timeline is selected.
    TimelineChange {
        #[serde(rename = "timeline")]
        timeline_name: TimelineName,

        #[serde(with = "serde::time_real")]
        time: TimeReal,
    },

    /// Fired when the selection changes.
    ///
    /// This event is fired each time any part of the event payload changes,
    /// this includes for example clicking on different parts of the same
    /// entity in a 2D or 3D view.
    SelectionChange { items: Vec<SelectionChangeItem> },
}

/// A single item in a selection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum SelectionChangeItem {
    /// Selected an entity, or an instance of an entity.
    ///
    /// If the entity was selected within a view, then this also
    /// includes the view's name.
    ///
    /// If the entity was selected within a 2D or 3D space view,
    /// then this also includes the position.
    Entity {
        #[serde(with = "serde::entity_path")]
        entity_path: re_log_types::EntityPath,

        #[serde(with = "serde::instance_id")]
        #[serde(skip_serializing_if = "instance_is_all")]
        instance_id: re_log_types::Instance,

        #[serde(skip_serializing_if = "Option::is_none")]
        view_name: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        position: Option<glam::Vec3>,
    },

    /// Selected a view.
    View {
        #[serde(with = "serde::blueprint_id")]
        view_id: ViewId,
        view_name: String,
    },

    /// Selected a container.
    Container {
        #[serde(with = "serde::blueprint_id")]
        container_id: ContainerId,
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

impl SelectionChangeItem {
    pub fn new(
        item: &Item,
        context: &Option<ItemContext>,
        blueprint: &ViewportBlueprint,
    ) -> Option<Self> {
        match item {
            Item::StoreId(_)
            | Item::AppId(_)
            | Item::ComponentPath(_)
            | Item::DataSource(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_)
            | Item::TableId(_) => None,
            Item::View(view_id) => Some(Self::View {
                view_id: *view_id,
                view_name: if let Some(name) = get_view_name(blueprint, view_id) {
                    name
                } else {
                    re_log::debug!("failed to get view name for view id {view_id}");
                    return None;
                },
            }),
            Item::Container(container_id) => Some(Self::Container {
                container_id: *container_id,
                container_name: if let Some(name) = get_container_name(blueprint, container_id) {
                    name
                } else {
                    re_log::debug!("failed to get container name for container id {container_id}");
                    return None;
                },
            }),

            Item::DataResult(view_id, instance_path) => Some(Self::Entity {
                entity_path: instance_path.entity_path.clone(),
                instance_id: instance_path.instance,
                view_name: get_view_name(blueprint, view_id),
                position: get_position(context),
            }),
            Item::InstancePath(instance_path) => Some(Self::Entity {
                entity_path: instance_path.entity_path.clone(),
                instance_id: instance_path.instance,
                view_name: None,
                position: get_position(context),
            }),
        }
    }
}

pub type ViewerEventCallback = Rc<dyn Fn(ViewerEvent)>;

#[derive(Clone)]
pub struct ViewerEventDispatcher {
    f: ViewerEventCallback,
}

impl ViewerEventDispatcher {
    #[inline]
    pub fn new(f: ViewerEventCallback) -> Self {
        Self { f }
    }

    #[inline]
    pub fn on_play_state_change(&self, db: &EntityDb, playing: bool) {
        self.dispatch(ViewerEvent::from_db_and_kind(
            db,
            if playing {
                ViewerEventKind::Play
            } else {
                ViewerEventKind::Pause
            },
        ));
    }

    #[inline]
    pub fn on_time_update(&self, db: &EntityDb, time: TimeReal) {
        self.dispatch(ViewerEvent::from_db_and_kind(
            db,
            ViewerEventKind::TimeUpdate { time },
        ));
    }

    #[inline]
    pub fn on_timeline_change(&self, db: &EntityDb, timeline: Timeline, time: TimeReal) {
        self.dispatch(ViewerEvent::from_db_and_kind(
            db,
            ViewerEventKind::TimelineChange {
                timeline_name: *timeline.name(),
                time,
            },
        ));
    }

    #[inline]
    pub fn on_selection_change(
        &self,
        db: &EntityDb,
        items: &ItemCollection,
        viewport_blueprint: &ViewportBlueprint,
    ) {
        self.dispatch(ViewerEvent::from_db_and_kind(
            db,
            ViewerEventKind::SelectionChange {
                items: items
                    .iter()
                    .filter_map(|(item, ctx)| {
                        SelectionChangeItem::new(item, ctx, viewport_blueprint)
                    })
                    .collect(),
            },
        ));
    }

    #[inline]
    fn dispatch(&self, event: Option<ViewerEvent>) {
        if let Some(event) = event {
            (self.f)(event);
        }
    }
}

fn instance_is_all(v: &re_log_types::Instance) -> bool {
    v.is_all()
}

/// Customs serialization for event payloads.
///
/// We serialize events into JSON when crossing the js/py language bridge,
/// and some types don't serialize into something that would be
/// useful "as-is" in those languages.
mod serde {
    pub use ::serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub mod entity_path {
        use super::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S>(v: &re_log_types::EntityPath, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&v.to_string())
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<re_log_types::EntityPath, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s: String = Deserialize::deserialize(deserializer)?;
            re_log_types::EntityPath::parse_strict(&s).map_err(serde::de::Error::custom)
        }
    }

    pub mod instance_id {
        use super::{Deserialize, Deserializer, Serialize, Serializer};

        pub fn serialize<S>(v: &re_log_types::Instance, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Serialize::serialize(&v.specific_index().map(|v| v.get()), serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<re_log_types::Instance, D::Error>
        where
            D: Deserializer<'de>,
        {
            let v: Option<u64> = Deserialize::deserialize(deserializer)?;
            match v {
                Some(v) => Ok(re_log_types::Instance::from(v)),
                None => Ok(re_log_types::Instance::ALL),
            }
        }
    }

    pub mod blueprint_id {
        use super::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S, T>(
            v: &re_viewer_context::BlueprintId<T>,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
            T: re_viewer_context::BlueprintIdRegistry,
        {
            serializer.serialize_str(&v.uuid().to_string())
        }

        pub fn deserialize<'de, D, T>(
            deserializer: D,
        ) -> Result<re_viewer_context::BlueprintId<T>, D::Error>
        where
            D: Deserializer<'de>,
            T: re_viewer_context::BlueprintIdRegistry,
        {
            let s: String = Deserialize::deserialize(deserializer)?;
            re_types::external::uuid::Uuid::try_parse(&s)
                .map_err(serde::de::Error::custom)
                .map(re_viewer_context::BlueprintId::from)
        }
    }

    pub mod recording_id {
        use super::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S>(v: &re_log_types::StoreId, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(v.as_str())
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<re_log_types::StoreId, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s: String = Deserialize::deserialize(deserializer)?;
            Ok(re_log_types::StoreId::from_string(
                re_log_types::StoreKind::Recording,
                s,
            ))
        }
    }

    pub mod time_real {
        use super::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S>(v: &re_log_types::TimeReal, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_f64(v.as_f64())
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<re_log_types::TimeReal, D::Error>
        where
            D: Deserializer<'de>,
        {
            let v: f64 = Deserialize::deserialize(deserializer)?;
            Ok(re_log_types::TimeReal::from(v))
        }
    }
}
