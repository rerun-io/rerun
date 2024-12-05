use egui::WidgetText;

use re_chunk::EntityPath;
use re_data_ui::item_ui::{guess_instance_path_icon, guess_query_and_db_for_selected_entity};
use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_ui::{
    icons,
    syntax_highlighting::{InstanceInBrackets as InstanceWithBrackets, SyntaxHighlightedBuilder},
    SyntaxHighlighting as _,
};
use re_viewer_context::{
    contents_name_style, ContainerId, Contents, Item, SpaceViewId, ViewerContext,
};
use re_viewport_blueprint::ViewportBlueprint;

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
            Item::AppId(app_id) => {
                let title = app_id.to_string();
                Self::new(title, &icons::APPLICATION)
            }

            Item::DataSource(data_source) => {
                let title = data_source.to_string();
                Self::new(title, &icons::DATA_SOURCE)
            }

            Item::StoreId(store_id) => Self::from_store_id(ctx, store_id),

            Item::InstancePath(instance_path) => {
                Self::from_instance_path(ctx, style, instance_path)
            }

            Item::ComponentPath(component_path) => Self::from_component_path(ctx, component_path),

            Item::Container(container_id) => Self::from_container_id(viewport, container_id),

            Item::SpaceView(view_id) => Self::from_view_id(ctx, viewport, view_id),

            Item::DataResult(view_id, instance_path) => {
                let item_title = Self::from_instance_path(ctx, style, instance_path);
                if let Some(view) = viewport.view(view_id) {
                    item_title.with_tooltip(
                        SyntaxHighlightedBuilder::new(ctx.egui_ctx.style())
                            .append(instance_path)
                            .append(&format!(" in view '{}'", view.display_name_or_default())),
                    )
                } else {
                    item_title
                }
            }
        }
    }

    pub fn from_store_id(ctx: &ViewerContext<'_>, store_id: &re_log_types::StoreId) -> Self {
        let id_str = format!("{} ID: {}", store_id.kind, store_id);

        let title = if let Some(entity_db) = ctx.store_context.bundle.get(store_id) {
            if let Some(info) = entity_db.store_info() {
                let time = info
                    .started
                    .format_time_custom("[hour]:[minute]:[second]", ctx.app_options.time_zone)
                    .unwrap_or("<unknown time>".to_owned());

                format!("{} - {}", info.application_id, time)
            } else {
                id_str.clone()
            }
        } else {
            id_str.clone()
        };

        let icon = match store_id.kind {
            re_log_types::StoreKind::Recording => &icons::RECORDING,
            re_log_types::StoreKind::Blueprint => &icons::BLUEPRINT,
        };

        Self::new(title, icon).with_tooltip(id_str)
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
        let entity_path = &component_path.entity_path;
        let component_name = &component_path.component_name;

        let (_query, db) = guess_query_and_db_for_selected_entity(ctx, entity_path);
        let is_static = db
            .storage_engine()
            .store()
            .entity_has_static_component(entity_path, component_name);

        Self::new(
            component_name.short_name(),
            if is_static {
                &icons::COMPONENT_STATIC
            } else {
                &icons::COMPONENT_TEMPORAL
            },
        )
        .with_tooltip(format!(
            "{} component {} of entity '{}'",
            if is_static { "Static" } else { "Temporal" },
            component_name.full_name(),
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
            Contents::SpaceView(view_id) => Self::from_view_id(ctx, viewport, view_id),
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
                &icons::SPACE_VIEW_UNKNOWN,
            )
            .with_tooltip("Failed to find container in blueprint")
        }
    }

    fn from_view_id(
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        view_id: &SpaceViewId,
    ) -> Self {
        if let Some(view) = viewport.view(view_id) {
            let view_class = view.class(ctx.space_view_class_registry);

            let hover_text = if let Some(display_name) = view.display_name.as_ref() {
                format!("{} view {display_name:?}", view_class.display_name(),)
            } else {
                format!("{} view", view_class.display_name())
            };

            let view_name = view.display_name_or_default();

            Self::new(
                view_name.as_ref(),
                view.class(ctx.space_view_class_registry).icon(),
            )
            .with_label_style(contents_name_style(&view_name))
            .with_tooltip(hover_text)
        } else {
            Self::new(
                format!("Unknown view {view_id}"),
                &icons::SPACE_VIEW_UNKNOWN,
            )
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
