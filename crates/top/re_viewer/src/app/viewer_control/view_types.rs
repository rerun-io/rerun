use re_protos::viewer_control::v1alpha1::{ListViewTypesResponse, ViewerViewType};
use re_types_core::reflection::Reflection;
use re_viewer_context::ViewClassRegistry;

pub fn list_view_types(
    registry: &ViewClassRegistry,
    reflection: &Reflection,
) -> ListViewTypesResponse {
    let mut view_types = registry
        .iter_registry()
        .map(|entry| {
            let metadata = reflection.views.get(&entry.identifier);
            ViewerViewType {
                class_identifier: entry.identifier.to_string(),
                display_name: entry.class.display_name().to_owned(),
                description: metadata
                    .map(|view| view.description.clone())
                    .unwrap_or_default(),
                keywords: metadata
                    .map(|view| view.keywords.clone())
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    view_types.sort_by(|a, b| a.class_identifier.cmp(&b.class_identifier));
    ListViewTypesResponse { view_types }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Built-in views expose discovery metadata in class-identifier order.
    #[test]
    fn builtins_have_discovery_metadata() {
        let mut ctx = re_test_context::TestContext::new();
        let registry = crate::default_views::create_view_class_registry(
            &ctx.reflection,
            &ctx.app_options,
            &mut ctx.component_fallback_registry,
        )
        .unwrap();
        let reflection = ctx.reflection.read();
        let response = list_view_types(&registry, &reflection);
        assert_eq!(response.view_types.len(), registry.iter_registry().count());
        assert!(
            response
                .view_types
                .windows(2)
                .all(|pair| pair[0].class_identifier < pair[1].class_identifier)
        );
        for view in &response.view_types {
            assert!(!view.description.is_empty(), "{}", view.class_identifier);
            assert!(!view.keywords.is_empty(), "{}", view.class_identifier);
        }
        let raw = response
            .view_types
            .iter()
            .find(|view| view.class_identifier == "RawData")
            .unwrap();
        assert!(raw.description.contains("single entity"));
        assert!(raw.keywords.iter().any(|keyword| keyword == "message"));
        let dataframe = response
            .view_types
            .iter()
            .find(|view| view.class_identifier == "Dataframe")
            .unwrap();
        assert!(dataframe.description.contains("tabular"));
    }

    /// Only registered views are listed, even when reflection metadata is unavailable.
    #[test]
    fn only_registered_views_are_listed_and_missing_metadata_is_allowed() {
        let ctx =
            re_test_context::TestContext::new_with_view_class::<re_view_raw_data::RawDataView>();
        let response = list_view_types(&ctx.view_class_registry, &ctx.reflection.read());
        assert_eq!(response.view_types.len(), 1);
        assert_eq!(response.view_types[0].class_identifier, "RawData");

        let mut reflection = ctx.reflection.read().clone();
        reflection.views.clear();
        let response = list_view_types(&ctx.view_class_registry, &reflection);
        assert_eq!(response.view_types[0].display_name, "Raw data");
        assert!(response.view_types[0].description.is_empty());
        assert!(response.view_types[0].keywords.is_empty());
    }
}
