//! Builder API for constructing lenses.

use std::collections::BTreeMap;

use re_chunk::{ComponentIdentifier, EntityPath, TimelineName};
use re_log_types::TimeType;
use re_sdk_types::ComponentDescriptor;

use crate::selector::DynExpr;
use crate::{LensBuilderError, Selector, ast};

/// Builder for lenses with support for multiple output modes.
#[must_use]
pub struct LensBuilder {
    input: ComponentIdentifier,
    scatter: bool,
    same_entity_output: Option<ast::LensOutput>,
    entity_outputs: BTreeMap<EntityPath, ast::LensOutput>,
}

impl LensBuilder {
    pub(crate) fn new(component: impl Into<ComponentIdentifier>) -> Self {
        Self {
            input: component.into(),
            scatter: false,
            same_entity_output: None,
            entity_outputs: BTreeMap::new(),
        }
    }

    /// Enables 1:N row mapping (scatter) for this lens.
    ///
    /// When scatter is enabled, each input row produces multiple output rows by
    /// exploding list arrays. The timepoint is replicated/scattered across the
    /// output rows. Useful for flattening lists or exploding batches.
    ///
    /// By default, lenses use 1:1 row mapping where each input row produces
    /// exactly one output row.
    pub fn scatter(mut self) -> Self {
        self.scatter = true;
        self
    }

    /// Adds an output with the same entity as the input.
    ///
    /// Each input row produces exactly one output row (unless `.scatter()` is set on the
    /// builder). Outputs inherit time columns from the input, plus any additional time
    /// columns specified via `.time()`.
    pub fn output_columns(
        mut self,
        builder: impl FnOnce(OutputBuilder) -> Result<OutputBuilder, LensBuilderError>,
    ) -> Result<Self, LensBuilderError> {
        if self.same_entity_output.is_some() {
            return Err(LensBuilderError::DuplicateSameEntityOutput);
        }
        let output_builder = OutputBuilder::new();
        let output = builder(output_builder)?.build(self.input)?;
        self.same_entity_output = Some(output);
        Ok(self)
    }

    /// Adds an output targeting an explicit entity path.
    ///
    /// Each input row produces exactly one output row (unless `.scatter()` is set on the
    /// builder). Outputs inherit time columns from the input, plus any additional time
    /// columns specified via `.time()`.
    pub fn output_columns_at(
        mut self,
        entity_path: impl Into<EntityPath>,
        builder: impl FnOnce(OutputBuilder) -> Result<OutputBuilder, LensBuilderError>,
    ) -> Result<Self, LensBuilderError> {
        let entity_path = entity_path.into();
        if self.entity_outputs.contains_key(&entity_path) {
            return Err(LensBuilderError::DuplicateTargetEntity {
                target_entity: entity_path,
            });
        }
        let output_builder = OutputBuilder::new();
        let output = builder(output_builder)?.build(self.input)?;
        self.entity_outputs.insert(entity_path, output);
        Ok(self)
    }

    /// Finalizes this builder and returns the corresponding lens.
    pub fn build(self) -> ast::Lens {
        ast::Lens {
            input: self.input,
            scatter: self.scatter,
            same_entity_output: self.same_entity_output,
            entity_outputs: self.entity_outputs,
        }
    }
}

// ==================== Output Builder ====================

/// Builder for lens output groups.
///
/// Defines the component and time columns that a lens output produces.
#[must_use]
pub struct OutputBuilder {
    components: Vec<ast::ComponentOutput>,
    time_outputs: Vec<ast::TimeOutput>,
}

// TODO(RR-3962): Get rid of the `unnecessary_wraps`.
#[expect(
    clippy::unnecessary_wraps,
    reason = "Result return enables `?` chaining in builder closures"
)]
impl OutputBuilder {
    fn new() -> Self {
        Self {
            components: vec![],
            time_outputs: vec![],
        }
    }

    /// Adds a component output column.
    ///
    /// # Arguments
    /// * `component_descr` - The descriptor for the output component
    /// * `selector` - Selector to apply to the input column
    pub fn component(
        mut self,
        component_descr: ComponentDescriptor,
        selector: impl Into<Selector<DynExpr>>,
    ) -> Result<Self, LensBuilderError> {
        self.components.push(ast::ComponentOutput {
            component_descr,
            selector: selector.into(),
        });
        Ok(self)
    }

    /// Adds a time extraction.
    ///
    /// Extracts data from the input column to create a new time column for the output rows.
    ///
    /// # Arguments
    /// * `timeline_name` - Name of the timeline to create
    /// * `timeline_type` - Type of timeline (Sequence or Time)
    /// * `selector` - Selector to extract time values (must produce [`arrow::array::Int64Array`])
    pub fn time(
        mut self,
        timeline_name: impl Into<TimelineName>,
        timeline_type: TimeType,
        selector: impl Into<Selector<DynExpr>>,
    ) -> Result<Self, LensBuilderError> {
        self.time_outputs.push(ast::TimeOutput {
            timeline_name: timeline_name.into(),
            timeline_type,
            selector: selector.into(),
        });
        Ok(self)
    }

    /// Builds a [`ast::LensOutput`], the `input` is passed for providing contextualized errors.
    fn build(self, input: ComponentIdentifier) -> Result<ast::LensOutput, LensBuilderError> {
        let components = self.components.try_into().map_err(|_err| {
            LensBuilderError::MissingOutputComponent {
                input_component: input,
            }
        })?;

        Ok(ast::LensOutput {
            input_id: input,
            output_components: components,
            output_timelines: self.time_outputs,
        })
    }
}
