use re_data_ui::item_ui::{guess_instance_path_icon, guess_query_and_db_for_selected_entity};
use re_ui::{icons, list_item, DesignTokens, SyntaxHighlighting as _, UiExt as _};
use re_viewer_context::{contents_name_style, Item, SystemCommandSender as _, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

#[must_use]
pub struct ItemTitle {
    name: egui::WidgetText,
    hover: Option<String>,
    icon: &'static re_ui::Icon,
    label_style: Option<re_ui::LabelStyle>,
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

            Item::StoreId(store_id) => {
                let id_str = format!("{} ID: {}", store_id.kind, store_id);

                let title = if let Some(entity_db) = ctx.store_context.bundle.get(store_id) {
                    if let Some(info) = entity_db.store_info() {
                        let time = info
                            .started
                            .format_time_custom(
                                "[hour]:[minute]:[second]",
                                ctx.app_options.time_zone,
                            )
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

            Item::InstancePath(instance_path) => {
                let typ = item.kind();
                let name = instance_path.syntax_highlighted(style);

                Self::new(name, guess_instance_path_icon(ctx, instance_path))
                    .with_tooltip(format!("{typ} '{instance_path}'"))
            }

            Item::ComponentPath(component_path) => {
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

            Item::Container(container_id) => {
                if let Some(container_blueprint) = viewport.container(container_id) {
                    let hover_text =
                        if let Some(display_name) = container_blueprint.display_name.as_ref() {
                            format!(
                                "{:?} container {display_name:?}",
                                container_blueprint.container_kind,
                            )
                        } else {
                            format!("Unnamed {:?} container", container_blueprint.container_kind,)
                        };

                    let container_name = container_blueprint.display_name_or_default();
                    Self::new(
                        container_name.as_ref(),
                        re_viewer_context::icon_for_container_kind(
                            &container_blueprint.container_kind,
                        ),
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

            Item::SpaceView(view_id) => {
                if let Some(view) = viewport.view(view_id) {
                    let view_class = view.class(ctx.space_view_class_registry);

                    let hover_text = if let Some(display_name) = view.display_name.as_ref() {
                        format!(
                            "Space view {:?} of type {}",
                            display_name,
                            view_class.display_name()
                        )
                    } else {
                        format!("Unnamed view of type {}", view_class.display_name())
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

            Item::DataResult(view_id, instance_path) => {
                let name = instance_path.syntax_highlighted(style);

                let item_title = Self::new(name, guess_instance_path_icon(ctx, instance_path));

                if let Some(view) = viewport.view(view_id) {
                    let typ = item.kind();
                    item_title.with_tooltip(format!(
                        "{typ} '{instance_path}' as shown in view {:?}",
                        view.display_name
                    ))
                } else {
                    item_title
                }
            }
        }
    }

    fn new(name: impl Into<egui::WidgetText>, icon: &'static re_ui::Icon) -> Self {
        Self {
            name: name.into(),
            hover: None,
            icon,
            label_style: None,
        }
    }

    #[inline]
    fn with_tooltip(mut self, hover: impl Into<String>) -> Self {
        self.hover = Some(hover.into());
        self
    }

    #[inline]
    fn with_label_style(mut self, label_style: re_ui::LabelStyle) -> Self {
        self.label_style = Some(label_style);
        self
    }

    pub fn ui(self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui, item: &Item) {
        let Self {
            name,
            hover,
            icon,
            label_style,
        } = self;

        let mut content = list_item::LabelContent::new(name).with_icon(icon);

        if let Some(label_style) = label_style {
            content = content.label_style(label_style);
        }

        let response = ui
            .list_item()
            .with_height(DesignTokens::title_bar_height())
            .selected(true)
            .show_flat(ui, content);

        if response.clicked() {
            // If the user has multiple things selected but only wants to have one thing selected,
            // this is how they can do it.
            ctx.command_sender
                .send_system(re_viewer_context::SystemCommand::SetSelection(item.clone()));
        }

        if let Some(hover) = hover {
            response.on_hover_text(hover);
        }
    }
}
