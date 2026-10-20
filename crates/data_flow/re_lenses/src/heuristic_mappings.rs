//! Reflection-backed mapping candidates derived from registered lenses.

use re_sdk_types::reflection::ComponentIdentifierReflectionMap;

/// Enriches reflection from the built-in lenses without replacing existing metadata.
pub fn register_heuristic_mappings(
    reflection: &mut ComponentIdentifierReflectionMap,
) -> Result<(), crate::LensBuilderError> {
    let lenses = crate::Lenses::new(crate::OutputMode::ForwardUnmatched);
    #[cfg(feature = "semantic")]
    let lenses = crate::semantic::ros2msg::all()?
        .into_iter()
        .fold(lenses, crate::Lenses::add_lens);
    lenses.register_heuristic_component_mappings(reflection);
    Ok(())
}

#[cfg(all(test, feature = "semantic"))]
mod tests {
    #[test]
    fn snapshot_builtin_mapping_inventory() {
        re_log::setup_logging();
        let _log_guard = re_log::PanicOnWarnScope::new();
        let original = &re_sdk_types::reflection::reflection().component_identifiers;
        let mut reflection = original.clone();
        super::register_heuristic_mappings(&mut reflection).expect("built-in lenses must build");

        #[expect(clippy::iter_over_hash_type)] // Each descriptor is checked independently.
        for (component, before) in original {
            assert_eq!(reflection[component].descriptor(), before.descriptor());
        }
        let mappings: std::collections::BTreeMap<_, std::collections::BTreeMap<_, _>> = reflection
            .iter()
            .filter(|(_, entry)| !entry.heuristic_mappings().is_empty())
            .map(|(source, entry)| {
                (
                    source.to_string(),
                    entry
                        .heuristic_mappings()
                        .iter()
                        .map(|mapping| {
                            (
                                mapping.descriptor.component.to_string(),
                                mapping.selector.clone(),
                            )
                        })
                        .collect(),
                )
            })
            .collect();
        insta::assert_debug_snapshot!("builtin_mapping_inventory", mappings);
    }
}
