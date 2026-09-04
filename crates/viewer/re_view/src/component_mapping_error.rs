use std::sync::Arc;

use re_log_types::external::arrow;
use re_types_core::ComponentIdentifier;

/// Error that can occur when mapping components.
#[derive(thiserror::Error, Debug, Clone)]
pub enum ComponentMappingError {
    /// Failed to parse a selector.
    #[error("Failed to parse selector: {0}")]
    SelectorParseFailed(re_lenses_core::SelectorError),

    /// Failed to execute a selector.
    #[error("Failed to select data: {0}")]
    SelectorExecutionFailed(re_lenses_core::SelectorError),

    /// Failed to cast component data to target datatype.
    #[error("Failed to cast from {source_datatype} to {target_datatype}: {err}")]
    CastFailed {
        source_datatype: arrow::datatypes::DataType,
        target_datatype: arrow::datatypes::DataType,
        err: Arc<arrow::error::ArrowError>,
    },

    #[error("No override is available for component '{0}'.")]
    OverrideUnavailable(ComponentIdentifier),

    #[error("Component '{component}' does not exist on the entity.")]
    ComponentNotPresentOnEntity {
        component: ComponentIdentifier,
        components_with_same_suffix: Vec<ComponentIdentifier>,
    },

    #[error("Component '{0}' exists on the entity but no data is available at the given time.")]
    NoComponentDataForQuery(ComponentIdentifier),

    // Note that we don't know whether we're actively fetching data for it.
    #[error("Component '{0}' exists on the entity but data for it hasn't been loaded yet.")]
    NoComponentDataForQueryButIsFetchable(ComponentIdentifier),
}

impl ComponentMappingError {
    pub fn component_not_present_on_entity(
        component: ComponentIdentifier,
        available_components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> Self {
        let components_with_same_suffix = available_components
            .into_iter()
            .filter(|available| {
                available != &component && available.as_str().ends_with(component.as_str())
            })
            .collect();

        Self::ComponentNotPresentOnEntity {
            component,
            components_with_same_suffix,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::SelectorParseFailed(_) => "Failed to parse selector.".to_owned(),
            Self::SelectorExecutionFailed(_) => "Failed to select data.".to_owned(),
            Self::CastFailed {
                source_datatype,
                target_datatype,
                ..
            } => {
                format!("Failed to cast from {source_datatype} to {target_datatype}.")
            }
            Self::OverrideUnavailable(_)
            | Self::ComponentNotPresentOnEntity { .. }
            | Self::NoComponentDataForQuery(_)
            | Self::NoComponentDataForQueryButIsFetchable(_) => self.to_string(),
        }
    }

    pub fn details(&self) -> Option<String> {
        match self {
            Self::SelectorParseFailed(err) | Self::SelectorExecutionFailed(err) => {
                Some(err.to_string())
            }
            Self::CastFailed { err, .. } => Some(err.to_string()),
            Self::ComponentNotPresentOnEntity {
                components_with_same_suffix: similar_components,
                ..
            } if !similar_components.is_empty() => {
                let listed_components = similar_components
                    .iter()
                    .take(3)
                    .map(|component| format!("'{}'", component.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                let ending = if similar_components.len() > 3 {
                    ", …"
                } else {
                    "."
                };
                Some(format!(
                    "Components with a matching suffix: {listed_components}{ending}"
                ))
            }
            Self::OverrideUnavailable(_)
            | Self::ComponentNotPresentOnEntity { .. }
            | Self::NoComponentDataForQuery(_)
            | Self::NoComponentDataForQueryButIsFetchable(_) => None,
        }
    }
}

#[test]
fn component_not_present_suggests_namespaced_match() {
    let error = ComponentMappingError::component_not_present_on_entity(
        "GateDecision:window_start_ms".into(),
        [
            "rerun.components.Position3D".into(),
            "syrinx.archetypes.GateDecision:window_start_ms".into(),
        ],
    );

    assert_eq!(
        error.details().as_deref(),
        Some(
            "Components with a matching suffix: \
             'syrinx.archetypes.GateDecision:window_start_ms'."
        )
    );
}

#[test]
fn component_not_present_limits_printed_suggestions() {
    let error = ComponentMappingError::component_not_present_on_entity(
        "field".into(),
        [
            "one.field".into(),
            "two.field".into(),
            "three.field".into(),
            "four.field".into(),
        ],
    );

    let ComponentMappingError::ComponentNotPresentOnEntity {
        components_with_same_suffix: similar_components,
        ..
    } = &error
    else {
        unreachable!();
    };
    assert_eq!(similar_components.len(), 4);
    assert_eq!(
        error.details().as_deref(),
        Some("Components with a matching suffix: 'one.field', 'two.field', 'three.field', …")
    );
}
