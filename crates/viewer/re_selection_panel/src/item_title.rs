use egui::WidgetText;
use re_chunk::EntityPath;
use re_data_ui::item_ui::{guess_instance_path_icon, guess_query_and_db_for_selected_entity};
use re_entity_db::InstancePath;
use re_log_types::{ComponentPath, TableId};
use re_sdk_types::archetypes::RecordingInfo;
use re_sdk_types::components::Timestamp;
use re_ui::syntax_highlighting::{
    InstanceInBrackets as InstanceWithBrackets, SyntaxHighlightedBuilder,
};
use re_ui::{SyntaxHighlighting as _, icons};
use re_viewer_context::{
    ContainerId, Contents, DataResultInteractionAddress, Item, ViewId, ViewerContext,
    contents_name_style,
};
use re_viewport_blueprint::ViewportBlueprint;

pub fn is_component_static(ctx: &ViewerContext<'_>, component_path: &ComponentPath) -> bool {
    let ComponentPath {
        entity_path,
        component,
    } = component_path;
    let (_query, db) = guess_query_and_db_for_selected_entity(ctx, entity_path);
    db.storage_engine()
        .store()
        .entity_has_static_component(entity_path, *component)
}

#[must_use]
pub struct ItemTitle {
    pub icon: &'static re_ui::Icon,
    pub label: egui::WidgetText,
    pub label_style: Option<re_ui::LabelStyle>,
    pub tooltip: Option<WidgetText>,
}

impl ItemTitle {
    pub fn from_item(
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        style: &egui::Style,
        item: &Item,
    ) -> Self {
        match &item {
            Item::AppId(app_id) => Self::new(app_id.to_string(), &icons::APPLICATION),

            Item::DataSource(data_source) => {
                Self::new(data_source.to_string(), &icons::DATA_SOURCE)
            }

            Item::StoreId(store_id) => Self::from_store_id(ctx, store_id),
            Item::TableId(table_id) => Self::from_table_id(ctx, table_id),

            Item::InstancePath(instance_path) => {
                Self::from_instance_path(ctx, style, instance_path)
            }

            Item::ComponentPath(component_path) => Self::from_component_path(ctx, component_path),

            Item::Container(container_id) => Self::from_container_id(viewport, container_id),

            Item::View(view_id) => Self::from_view_id(ctx, viewport, view_id),

            Item::DataResult(DataResultInteractionAddress {
                view_id,
                instance_path,
                visualizer: _, // Can't distinguish visualizer here since we don't name them.
            }) => {
                let item_title = Self::from_instance_path(ctx, style, instance_path);
                if let Some(view) = viewport.view(view_id) {
                    item_title.with_tooltip(
                        SyntaxHighlightedBuilder::new()
                            .with(instance_path)
                            .with_body(" in view ")
                            .with(&view.display_name_or_default())
                            .into_widget_text(&ctx.egui_ctx().global_style()),
                    )
                } else {
                    item_title
                }
            }

            // TODO(#10566): There should be an `EntryName` in this `Item` arm.
            Item::RedapEntry(entry) => Self::new(entry.entry_id.to_string(), &icons::DATASET),

            // TODO(lucasmerlin): Icon?
            Item::RedapServer(origin) => Self::new(origin.to_string(), &icons::DATASET),
        }
    }

    pub fn from_table_id(_ctx: &ViewerContext<'_>, table_id: &TableId) -> Self {
        Self::new(table_id.as_str(), &icons::ENTITY_RESERVED).with_tooltip(table_id.as_str())
    }

    pub fn from_store_id(ctx: &ViewerContext<'_>, store_id: &re_log_types::StoreId) -> Self {
        let title = if let Some(entity_db) = ctx.store_bundle().get(store_id) {
            if let Some(started) = entity_db.recording_info_property::<Timestamp>(
                RecordingInfo::descriptor_start_time().component,
            ) {
                let time = re_log_types::Timestamp::from(started.0)
                    .format_time_compact(ctx.app_options().timestamp_format);
                format!("{} - {time}", store_id.application_id())
            } else {
                store_id.application_id().to_string()
            }
        } else {
            store_id.application_id().to_string()
        };

        let icon = match store_id.kind() {
            re_log_types::StoreKind::Recording => &icons::RECORDING,
            re_log_types::StoreKind::Blueprint => &icons::BLUEPRINT,
        };

        Self::new(title, icon).with_tooltip(format!(
            "Store kind: {}\nApplication ID: {}\nRecording ID: {}",
            store_id.kind(),
            store_id.application_id(),
            store_id.recording_id(),
        ))
    }

    pub fn from_instance_path(
        ctx: &ViewerContext<'_>,
        style: &egui::Style,
        instance_path: &InstancePath,
    ) -> Self {
        let InstancePath {
            entity_path,
            instance,
        } = instance_path;

        let name = if instance.is_all() {
            // Entity path
            if let Some(last) = entity_path.last() {
                last.syntax_highlighted(style)
            } else {
                EntityPath::root().syntax_highlighted(style)
            }
        } else {
            // Instance path
            InstanceWithBrackets(*instance).syntax_highlighted(style)
        };

        Self::new(name, guess_instance_path_icon(ctx, instance_path))
            .with_tooltip(instance_path.syntax_highlighted(style))
    }

    pub fn from_component_path(ctx: &ViewerContext<'_>, component_path: &ComponentPath) -> Self {
        let is_static = is_component_static(ctx, component_path);

        let ComponentPath {
            entity_path,
            component,
        } = component_path;

        Self::new(
            component.as_str(),
            if is_static {
                &icons::COMPONENT_STATIC
            } else {
                &icons::COMPONENT_TEMPORAL
            },
        )
        .with_tooltip(format!(
            "{} component {} of entity '{}'",
            if is_static { "Static" } else { "Temporal" },
            component,
            entity_path
        ))
    }

    pub fn from_contents(
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        contents: &Contents,
    ) -> Self {
        match contents {
            Contents::Container(container_id) => Self::from_container_id(viewport, container_id),
            Contents::View(view_id) => Self::from_view_id(ctx, viewport, view_id),
        }
    }

    pub fn from_container_id(viewport: &ViewportBlueprint, container_id: &ContainerId) -> Self {
        if let Some(container_blueprint) = viewport.container(container_id) {
            let hover_text = if let Some(display_name) = container_blueprint.display_name.as_ref() {
                format!(
                    "{:?} container {display_name:?}",
                    container_blueprint.container_kind,
                )
            } else {
                format!("{:?} container", container_blueprint.container_kind,)
            };

            let container_name = container_blueprint.display_name_or_default();
            Self::new(
                container_name.as_ref(),
                re_viewer_context::icon_for_container_kind(&container_blueprint.container_kind),
            )
            .with_label_style(contents_name_style(&container_name))
            .with_tooltip(hover_text)
        } else {
            Self::new(
                format!("Unknown container {container_id}"),
                &icons::VIEW_UNKNOWN,
            )
            .with_tooltip("Failed to find container in blueprint")
        }
    }

    fn from_view_id(
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        view_id: &ViewId,
    ) -> Self {
        if let Some(view) = viewport.view(view_id) {
            let view_class = view.class(ctx.view_class_registry());

            let hover_text = if let Some(display_name) = view.display_name.as_ref() {
                format!("{} view {display_name:?}", view_class.display_name(),)
            } else {
                format!("{} view", view_class.display_name())
            };

            let view_name = view.display_name_or_default();

            Self::new(
                view_name.as_ref(),
                view.class(ctx.view_class_registry()).icon(),
            )
            .with_label_style(contents_name_style(&view_name))
            .with_tooltip(hover_text)
        } else {
            Self::new(format!("Unknown view {view_id}"), &icons::VIEW_UNKNOWN)
                .with_tooltip("Failed to find view in blueprint")
        }
    }

    fn new(name: impl Into<egui::WidgetText>, icon: &'static re_ui::Icon) -> Self {
        Self {
            label: name.into(),
            tooltip: None,
            icon,
            label_style: None,
        }
    }

    #[inline]
    fn with_tooltip(mut self, tooltip: impl Into<WidgetText>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    #[inline]
    fn with_label_style(mut self, label_style: re_ui::LabelStyle) -> Self {
        self.label_style = Some(label_style);
        self
    }
}
