//! Builder API for constructing lenses.

use re_chunk::{ComponentIdentifier, EntityPath, TimelineName};
use re_log_types::TimeType;
use re_sdk_types::ComponentDescriptor;

use vec1::Vec1;

use crate::ast::{
    ComponentOutput, DeriveSameEntityLens, DeriveSeparateEntityLens, Lens, LensInner, MutateLens,
    Rows, TimeOutput,
};
use crate::selector::DynExpr;
use crate::{LensBuilderError, Selector};

/// Builder for a derive lens that creates new component/time columns from an input component.
#[must_use]
pub struct DeriveLensBuilder {
    input: ComponentIdentifier,
    rows: Rows,
    target_entity: Option<EntityPath>,
    components: Vec<ComponentOutput>,
    time_outputs: Vec<TimeOutput>,
}

impl DeriveLensBuilder {
    pub(crate) fn new(input: impl Into<ComponentIdentifier>) -> Self {
        Self {
            input: input.into(),
            rows: Rows::default(),
            target_entity: None,
            components: Vec::new(),
            time_outputs: Vec::new(),
        }
    }

    /// Enables 1:N row mapping (scatter/explode), where each input row can
    /// produce multiple output rows.
    pub fn scatter_rows(mut self) -> Self {
        self.rows = Rows::OneToMany;
        self
    }

    /// Sets the target entity path for the output.
    ///
    /// When set, output is written to this entity instead of the input entity.
    pub fn output_entity(mut self, entity: impl Into<EntityPath>) -> Self {
        self.target_entity = Some(entity.into());
        self
    }

    /// Adds a component output column.
    #[expect(
        clippy::wrong_self_convention,
        reason = "builder pattern: consumes self"
    )]
    pub fn to_component(
        mut self,
        component_descr: ComponentDescriptor,
        selector: impl Into<Selector<DynExpr>>,
    ) -> Self {
        self.components.push(ComponentOutput {
            component_descr,
            selector: selector.into(),
        });
        self
    }

    /// Adds a time extraction output.
    ///
    /// Extracts data from the input column to create a new time column.
    #[expect(
        clippy::wrong_self_convention,
        reason = "builder pattern: consumes self"
    )]
    pub fn to_timeline(
        mut self,
        timeline_name: impl Into<TimelineName>,
        timeline_type: TimeType,
        selector: impl Into<Selector<DynExpr>>,
    ) -> Self {
        self.time_outputs.push(TimeOutput {
            timeline_name: timeline_name.into(),
            timeline_type,
            selector: selector.into(),
        });
        self
    }

    /// Builds the derive lens.
    ///
    /// Fails if no component outputs were added, or if the output component
    /// matches the input on the same entity.
    pub fn build(self) -> Result<Lens, LensBuilderError> {
        let output_components = Vec1::try_from_vec(self.components).map_err(|_empty_vec| {
            LensBuilderError::MissingOutputComponent {
                input_component: self.input,
            }
        })?;

        if self.target_entity.is_none() {
            for comp in &output_components {
                if comp.component_descr.component == self.input {
                    return Err(LensBuilderError::InputEqualsOutput {
                        component: self.input,
                    });
                }
            }
        }

        let inner = if let Some(target_entity) = self.target_entity {
            LensInner::DeriveSeparateEntity(DeriveSeparateEntityLens {
                input: self.input,
                rows: self.rows,
                target_entity,
                output_components,
                output_timelines: self.time_outputs,
            })
        } else {
            LensInner::DeriveSameEntity(DeriveSameEntityLens {
                input: self.input,
                rows: self.rows,
                output_components,
                output_timelines: self.time_outputs,
            })
        };
        Ok(inner.into())
    }
}

/// Builder for a mutate lens that modifies the input component in-place.
#[must_use]
pub struct MutateLensBuilder {
    input: ComponentIdentifier,
    selector: Selector<DynExpr>,
    keep_row_ids: bool,
}

impl MutateLensBuilder {
    pub(crate) fn new(
        input: impl Into<ComponentIdentifier>,
        selector: impl Into<Selector<DynExpr>>,
    ) -> Self {
        Self {
            input: input.into(),
            selector: selector.into(),
            keep_row_ids: false,
        }
    }

    /// Preserves the original `RowIds` in the output chunk.
    pub fn keep_row_ids(mut self) -> Self {
        self.keep_row_ids = true;
        self
    }

    /// Builds the mutate lens.
    pub fn build(self) -> Lens {
        LensInner::Mutate(MutateLens {
            input: self.input,
            selector: self.selector,
            keep_row_ids: self.keep_row_ids,
        })
        .into()
    }
}
