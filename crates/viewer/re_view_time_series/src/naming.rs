use egui::ahash;
use re_log_types::EntityPath;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::archetypes::Scalars;
use re_sdk_types::blueprint::components::VisualizerInstructionId;

#[derive(Clone, Default)]
pub struct SeriesNamesContext {
    per_instruction_names: ahash::HashMap<VisualizerInstructionId, SeriesInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesInfo {
    pub entity_path: EntityPath,

    /// Empty string if the component was [`Scalars::descriptor_scalars`].
    pub component_with_selector: String,
}

impl SeriesInfo {
    pub fn new(entity_path: EntityPath, component: ComponentIdentifier, selector: &str) -> Self {
        fn extract_component_name(component: &ComponentIdentifier) -> String {
            let mut full_name = component.to_string();
            if let Some(idx) = full_name.rfind(':') {
                full_name.split_off(idx + 1)
            } else {
                full_name
            }
        }

        let component_with_selector =
            if component == Scalars::descriptor_scalars().component && selector.is_empty() {
                Default::default()
            } else {
                let component_name = extract_component_name(&component);
                format!("{component_name}{selector}")
            };

        Self {
            entity_path,
            component_with_selector,
        }
    }
}

impl SeriesNamesContext {
    pub fn insert(&mut self, id: VisualizerInstructionId, name: SeriesInfo) {
        self.per_instruction_names.insert(id, name);
    }

    /// Returns disambiguated names for all series.
    pub fn dissambiguated_names(&self) -> ahash::HashMap<VisualizerInstructionId, String> {
        re_tracing::profile_function!();

        // Get short entity paths for all unique entities
        let short_entity_paths = EntityPath::short_names_with_disambiguation(
            self.per_instruction_names
                .values()
                .map(|series| series.entity_path.clone())
                .collect::<nohash_hasher::IntSet<_>>(),
        );

        // Count occurrences of each component_with_selector
        let mut component_counts: ahash::HashMap<&str, usize> = ahash::HashMap::default();
        #[expect(clippy::iter_over_hash_type)]
        for series in self.per_instruction_names.values() {
            if !series.component_with_selector.is_empty() {
                *component_counts
                    .entry(&series.component_with_selector)
                    .or_insert(0) += 1;
            }
        }

        // Generate disambiguated names
        self.per_instruction_names
            .iter()
            .map(|(id, series)| {
                let name = if series.component_with_selector.is_empty() {
                    // Builtin Rerun scalar components - use entity path
                    short_entity_paths
                        .get(&series.entity_path)
                        .cloned()
                        .unwrap_or_else(|| "<unknown>".into())
                } else {
                    // Check if this component+selector combination appears multiple times
                    let count = component_counts
                        .get(series.component_with_selector.as_str())
                        .copied()
                        .unwrap_or(0);

                    if count > 1 {
                        // Duplicate - prefix with entity path
                        let short_name = short_entity_paths
                            .get(&series.entity_path)
                            .cloned()
                            .unwrap_or_else(|| "<unknown>".into());
                        format!("{}:{}", short_name, series.component_with_selector)
                    } else {
                        // Unique - just use component+selector
                        series.component_with_selector.clone()
                    }
                };
                (*id, name)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dissambiguated_names_unique_components() {
        // Test case: Different component+selector combinations should not be disambiguated
        let mut series_names = SeriesNamesContext::default();

        let id1 = VisualizerInstructionId::new_random();
        let id2 = VisualizerInstructionId::new_random();

        series_names.insert(
            id1,
            SeriesInfo::new(
                EntityPath::from("left/gripper"),
                ComponentIdentifier::from("MyArchetype:temperature"),
                "",
            ),
        );

        series_names.insert(
            id2,
            SeriesInfo::new(
                EntityPath::from("right/gripper"),
                ComponentIdentifier::from("MyArchetype:pressure"),
                "",
            ),
        );

        let result = series_names.dissambiguated_names();

        // Different components should not need disambiguation
        assert_eq!(result.get(&id1).map(|s| s.as_str()), Some("temperature"));
        assert_eq!(result.get(&id2).map(|s| s.as_str()), Some("pressure"));
    }

    #[test]
    fn test_dissambiguated_names_duplicate_components() {
        // Test case: Same component+selector from different entities should be disambiguated
        let mut series_names = SeriesNamesContext::default();

        let id1 = VisualizerInstructionId::new_random();
        let id2 = VisualizerInstructionId::new_random();

        series_names.insert(
            id1,
            SeriesInfo::new(
                EntityPath::from("left/gripper"),
                ComponentIdentifier::from("MyArchetype:message"),
                ".joints[]",
            ),
        );

        series_names.insert(
            id2,
            SeriesInfo::new(
                EntityPath::from("right/gripper"),
                ComponentIdentifier::from("MyArchetype:message"),
                ".joints[]",
            ),
        );

        let result = series_names.dissambiguated_names();

        // Same component+selector should be disambiguated with entity path
        assert_eq!(
            result.get(&id1).map(|s| s.as_str()),
            Some("left/gripper:message.joints[]")
        );
        assert_eq!(
            result.get(&id2).map(|s| s.as_str()),
            Some("right/gripper:message.joints[]")
        );
    }

    #[test]
    fn test_dissambiguated_names_builtin_scalars() {
        // Test case: Builtin scalars (empty component_with_selector) should use entity path
        let mut series_names = SeriesNamesContext::default();

        let id1 = VisualizerInstructionId::new_random();
        let id2 = VisualizerInstructionId::new_random();

        series_names.insert(
            id1,
            SeriesInfo::new(
                EntityPath::from("sensor/temperature"),
                Scalars::descriptor_scalars().component,
                "",
            ),
        );

        series_names.insert(
            id2,
            SeriesInfo::new(
                EntityPath::from("sensor/pressure"),
                Scalars::descriptor_scalars().component,
                "",
            ),
        );

        let result = series_names.dissambiguated_names();

        // Builtin scalars should use entity path
        assert_eq!(result.get(&id1).map(|s| s.as_str()), Some("temperature"));
        assert_eq!(result.get(&id2).map(|s| s.as_str()), Some("pressure"));
    }

    #[test]
    fn test_dissambiguated_names_mixed() {
        // Test case: Mix of unique and duplicate component+selector combinations
        let mut series_names = SeriesNamesContext::default();

        let id1 = VisualizerInstructionId::new_random();
        let id2 = VisualizerInstructionId::new_random();
        let id3 = VisualizerInstructionId::new_random();

        // Duplicate: message.joints[]
        series_names.insert(
            id1,
            SeriesInfo::new(
                EntityPath::from("left/gripper"),
                ComponentIdentifier::from("MyArchetype:message"),
                ".joints[]",
            ),
        );

        series_names.insert(
            id2,
            SeriesInfo::new(
                EntityPath::from("right/gripper"),
                ComponentIdentifier::from("MyArchetype:message"),
                ".joints[]",
            ),
        );

        // Unique: status.code
        series_names.insert(
            id3,
            SeriesInfo::new(
                EntityPath::from("system/controller"),
                ComponentIdentifier::from("MyArchetype:status"),
                ".code",
            ),
        );

        let result = series_names.dissambiguated_names();

        // Duplicates should be disambiguated
        assert_eq!(
            result.get(&id1).map(|s| s.as_str()),
            Some("left/gripper:message.joints[]")
        );
        assert_eq!(
            result.get(&id2).map(|s| s.as_str()),
            Some("right/gripper:message.joints[]")
        );

        // Unique should not be disambiguated
        assert_eq!(result.get(&id3).map(|s| s.as_str()), Some("status.code"));
    }
}
