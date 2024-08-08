use re_types::blueprint::datatypes::ComponentNames;
use re_types_core::{ComponentName, LoggableBatch as _};
use re_ui::UiExt;
use re_viewer_context::{MaybeMutRef, ViewerContext};
use std::collections::BTreeSet;

pub(crate) fn edit_query_component_name_list<
    CompT: re_types_core::Component + std::ops::DerefMut<Target = ComponentNames>,
>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, CompT>,
) -> egui::Response {
    let id_source = value.name().as_str();
    match value {
        MaybeMutRef::Ref(value) => view_component_name_list(ctx, ui, value, true),
        MaybeMutRef::MutRef(value) => edit_component_name_list(ctx, ui, value, true, id_source),
    }
}

pub(crate) fn edit_point_of_view_component_name_list<
    CompT: re_types_core::Component + std::ops::DerefMut<Target = ComponentNames>,
>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, CompT>,
) -> egui::Response {
    let id_source = value.name().as_str();
    match value {
        MaybeMutRef::Ref(value) => view_component_name_list(ctx, ui, value, false),
        MaybeMutRef::MutRef(value) => edit_component_name_list(ctx, ui, value, false, id_source),
    }
}

fn view_component_name_list(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &ComponentNames,
    empty_means_all: bool,
) -> egui::Response {
    ui.label(text_for_component_list(value, empty_means_all))
}

fn edit_component_name_list(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut ComponentNames,
    empty_means_all: bool,
    id_source: impl Into<egui::Id>,
) -> egui::Response {
    let mut selected_components: BTreeSet<ComponentName> = value.name_iter().collect();

    let result = egui::ComboBox::from_id_source(id_source.into())
        .selected_text(text_for_component_list(value, empty_means_all))
        .show_ui(ui, |ui| {
            let all_components = all_components(ctx);

            // handle the ALL case
            let mut empty_array = selected_components.is_empty();
            let response = ui.re_checkbox(
                &mut empty_array,
                if empty_means_all { "All" } else { "None" },
            );
            if response.changed() {
                if empty_array {
                    selected_components.clear();
                } else {
                    selected_components = all_components.clone();
                }
            }

            let mut any_changed = response.changed();
            ui.separator();

            for component in all_components {
                let mut selected = selected_components.contains(&component);
                let response = ui.re_checkbox(&mut selected, component.short_name());
                if response.changed() {
                    if selected {
                        selected_components.insert(component);
                    } else {
                        selected_components.remove(&component);
                    }
                }

                any_changed |= response.changed();
            }

            any_changed
        });

    let mut response = result.response;
    if result.inner.unwrap_or_default() {
        value.set_names(selected_components.into_iter());

        response.mark_changed()
    }
    response
}

fn text_for_component_list(value: &ComponentNames, empty_means_all: bool) -> egui::WidgetText {
    let names = &value.0;
    if names.is_empty() {
        if empty_means_all {
            "All".into()
        } else {
            "None".into()
        }
    } else if names.len() == 1 {
        ComponentName::from(names[0].as_str()).short_name().into()
    } else {
        format!("{} components", names.len()).into()
    }
}

// TODO(ab): it would make more sense to show only those components that are actually present in the
// current space view. However, we don't have enough context here to be able to do that.
fn all_components(ctx: &ViewerContext<'_>) -> BTreeSet<ComponentName> {
    ctx.recording_store()
        .iter_chunks()
        .flat_map(|chunk| chunk.component_names())
        .collect()
}
