//! Component mapping metadata from the row-preserving subset of a lens collection.

use re_sdk_types::ComponentDescriptor;
use re_sdk_types::reflection::{ComponentIdentifierReflectionMap, HeuristicMapping};

use crate::{CastTo, Lenses};

impl Lenses {
    /// Adds heuristic component mappings from these lenses to the reflection map,
    /// preserving existing component descriptors and heuristic mappings.
    ///
    /// Component mappings cannot represent entity filters, opaque operations, or explicit casts.
    /// Each representable output is registered independently.
    pub fn register_heuristic_component_mappings(
        &self,
        reflection: &mut ComponentIdentifierReflectionMap,
    ) {
        let all_entities = re_log_types::EntityPathFilter::all().resolve_without_substitutions();
        for (filter, lens) in &self.same_entity_derives {
            if filter != &all_entities || !lens.is_merge_candidate() {
                continue;
            }
            for output in &lens.output_components {
                if matches!(output.cast, Some(CastTo::Type(_))) {
                    continue;
                }
                let Some(selector) = output.selector.try_to_string() else {
                    continue;
                };
                let mapping = HeuristicMapping {
                    descriptor: output.component_descr.clone(),
                    selector,
                };
                let entry = reflection
                    .entry(lens.input)
                    .or_insert_with(|| ComponentDescriptor::partial(lens.input).into());
                if let Err(err) = entry.add_heuristic_mapping(mapping) {
                    re_log::error!("{err}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use re_sdk_types::archetypes::{CoordinateFrame, Measurements, Scalars};

    use super::*;
    use crate::{Lens, OutputMode, Selector};

    /// Lens inputs carry only an identifier, so registration must not replace an existing
    /// typed source descriptor with a partial one or discard previously registered mappings.
    /// Each lens output must append its own target descriptor and selector.
    #[test]
    fn registration_preserves_descriptor_and_existing_mappings() {
        let source = ComponentDescriptor::partial("custom:message")
            .with_component_type("custom.Message".into());
        let outputs = [
            Measurements::descriptor_values(),
            Measurements::descriptor_variances(),
            CoordinateFrame::descriptor_frame(),
        ];
        let selectors = [".value", ".variance", ".frame"];
        let lens = Lens::derive(source.component)
            .to_component(outputs[0].clone(), Selector::parse(selectors[0]).unwrap())
            .to_component(outputs[1].clone(), Selector::parse(selectors[1]).unwrap())
            .to_component(outputs[2].clone(), Selector::parse(selectors[2]).unwrap())
            .build()
            .unwrap();
        let lenses = Lenses::new(OutputMode::ForwardUnmatched).add_lens(lens);
        let mut reflection = ComponentIdentifierReflectionMap::default();
        let existing = HeuristicMapping {
            descriptor: Scalars::descriptor_scalars(),
            selector: ".other".to_owned(),
        };

        let mut entry: re_sdk_types::reflection::ComponentIdentifierReflection =
            source.clone().into();
        entry
            .add_heuristic_mapping(existing.clone())
            .expect("a new target should be accepted");
        reflection.insert(source.component, entry);
        lenses.register_heuristic_component_mappings(&mut reflection);

        assert_eq!(reflection[&source.component].descriptor(), &source);
        assert_eq!(
            reflection[&source.component].heuristic_mappings(),
            std::iter::chain(
                std::iter::once(existing),
                std::iter::zip(outputs, selectors).map(|(descriptor, selector)| HeuristicMapping {
                    descriptor,
                    selector: selector.to_owned(),
                }),
            )
            .collect::<Vec<_>>()
        );
    }

    /// Component mappings cannot express mutation, scattering, entity redirection or filtering,
    /// timeline outputs, explicit casts, or opaque selectors.
    /// An opaque selector on one output must not prevent a representable sibling output
    /// from being registered as a heuristic mapping.
    #[test]
    fn unrepresentable_outputs_do_not_hide_representable_mappings() {
        let output = Scalars::descriptor_scalars();
        let selector = Selector::parse(".").unwrap();
        let lenses = [
            // Not representable: mutates the input instead of deriving components.
            Lens::mutate("mutate:source", selector.clone()).build(),
            // Not representable: scatters rows into separate entities.
            Lens::scatter("scatter:source")
                .to_component(output.clone(), selector.clone())
                .build()
                .unwrap(),
            // Not representable: writes to a different entity.
            Lens::derive("other_entity:source")
                .output_entity("other")
                .to_component(output.clone(), selector.clone())
                .build()
                .unwrap(),
            // Not representable: the lens also produces a timeline output.
            Lens::derive("timeline:source")
                .to_component(output.clone(), selector.clone())
                .to_timeline("time", re_log_types::TimeType::Sequence, selector.clone())
                .build()
                .unwrap(),
            // Not representable: requires an explicit cast.
            Lens::derive("cast:source")
                .to_component_with_cast(
                    output.clone(),
                    selector.clone(),
                    CastTo::Type(arrow::datatypes::DataType::Float32),
                )
                .build()
                .unwrap(),
            Lens::derive("opaque:source")
                // Representable: an identity selector deriving a component on the same entity.
                .to_component(output.clone(), selector.clone())
                // Not representable: an opaque operation cannot be encoded as a selector string.
                .to_component(
                    Measurements::descriptor_values(),
                    selector
                        .clone()
                        .pipe(|input: &arrow::array::ArrayRef| Ok(Some(input.clone()))),
                )
                .build()
                .unwrap(),
        ]
        .into_iter()
        .fold(Lenses::new(OutputMode::ForwardUnmatched), Lenses::add_lens)
        // Not representable: the mapping would apply only to a subset of entities.
        .add_lens_with_filter(
            re_log_types::EntityPathFilter::parse_forgiving("/allowed/**"),
            Lens::derive("filtered:source")
                .to_component(output, selector)
                .build()
                .unwrap(),
        );

        let mut reflection = ComponentIdentifierReflectionMap::default();
        lenses.register_heuristic_component_mappings(&mut reflection);

        assert_eq!(reflection.len(), 1);
        assert_eq!(
            reflection[&"opaque:source".into()].heuristic_mappings(),
            vec![HeuristicMapping {
                descriptor: Scalars::descriptor_scalars(),
                selector: ".".to_owned(),
            }]
        );
    }
}
