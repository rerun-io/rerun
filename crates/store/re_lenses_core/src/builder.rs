//! Builder API for constructing lenses.

use re_chunk::{ComponentIdentifier, EntityPath, TimelineName};
use re_log_types::TimeType;
use re_sdk_types::ComponentDescriptor;

use crate::selector::DynExpr;
use crate::{LensError, Selector, ast};

/// Builder for lenses with support for multiple output modes.
#[must_use]
pub struct LensBuilder {
    input: ComponentIdentifier,
    outputs: Vec<ast::LensOutput>,
}

impl LensBuilder {
    pub(crate) fn new(component: impl Into<ComponentIdentifier>) -> Self {
        Self {
            input: component.into(),
            outputs: vec![],
        }
    }

    /// Adds an output group configured via the builder closure.
    ///
    /// When `scatter` is `false` (1:1), each input row produces exactly one output row.
    /// When `scatter` is `true` (1:N), each input row produces multiple output rows by
    /// exploding list arrays.
    ///
    /// Outputs inherit time columns from the input, plus any additional time columns
    /// specified via `.time()`.
    pub fn output(
        mut self,
        scatter: bool,
        builder: impl FnOnce(OutputBuilder) -> Result<OutputBuilder, LensError>,
    ) -> Result<Self, LensError> {
        let output_builder = OutputBuilder::new(scatter);
        let output = builder(output_builder)?.build(self.input)?;
        self.outputs.push(output);
        Ok(self)
    }

    /// Adds a temporal output with 1:1 row mapping.
    ///
    /// Each input row produces exactly one output row. Outputs inherit time columns from
    /// the input, plus any additional time columns specified via `.time()`.
    ///
    /// The output will use the same entity path as the input.
    pub fn output_columns(
        self,
        builder: impl FnOnce(OutputBuilder) -> Result<OutputBuilder, LensError>,
    ) -> Result<Self, LensError> {
        self.output(false, builder)
    }

    /// Adds a temporal output with 1:N row mapping (scatter).
    ///
    /// Each input row produces multiple output rows at the same timepoint. The timepoint
    /// is replicated/scattered across the output rows. Useful for flattening lists or
    /// exploding batches.
    ///
    /// The output will use the same entity path as the input.
    pub fn output_scatter_columns(
        self,
        builder: impl FnOnce(OutputBuilder) -> Result<OutputBuilder, LensError>,
    ) -> Result<Self, LensError> {
        self.output(true, builder)
    }

    /// Finalizes this builder and returns the corresponding lens.
    pub fn build(self) -> ast::Lens {
        ast::Lens {
            input: self.input,
            outputs: self.outputs,
        }
    }
}

// ==================== Output Builder ====================

/// Builder for lens output groups.
///
/// When `scatter` is `false` (1:1), each input row produces exactly one output row.
/// Outputs inherit time columns from the input, plus any additional time columns specified.
///
/// When `scatter` is `true` (1:N), each input row produces multiple output rows at the
/// same timepoint. The timepoint is replicated/scattered across all output rows. This is
/// useful for flattening lists or exploding batches while maintaining temporal alignment.
#[must_use]
pub struct OutputBuilder {
    scatter: bool,
    target_entity: ast::TargetEntity,
    components: Vec<ast::ComponentOutput>,
    time_outputs: Vec<ast::TimeOutput>,
}

// TODO(RR-3962): Get rid of the `unnecessary_wraps`.
#[expect(
    clippy::unnecessary_wraps,
    reason = "Result return enables `?` chaining in builder closures"
)]
impl OutputBuilder {
    fn new(scatter: bool) -> Self {
        Self {
            scatter,
            target_entity: ast::TargetEntity::SameAsInput,
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
    ) -> Result<Self, LensError> {
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
    ) -> Result<Self, LensError> {
        self.time_outputs.push(ast::TimeOutput {
            timeline_name: timeline_name.into(),
            timeline_type,
            selector: selector.into(),
        });
        Ok(self)
    }

    /// Specify the target entity path for the outputs.
    pub fn at_entity(mut self, entity_path: impl Into<EntityPath>) -> Self {
        self.target_entity = ast::TargetEntity::Explicit(entity_path.into());
        self
    }

    /// Builds a [`ast::LensOutput`], the `input` is passed for providing contextualized errors.
    fn build(self, input: ComponentIdentifier) -> Result<ast::LensOutput, LensError> {
        let components =
            self.components
                .try_into()
                .map_err(|_err| LensError::MissingOutputComponent {
                    input_component: input,
                })?;

        Ok(ast::LensOutput {
            scatter: self.scatter,
            target_entity: self.target_entity,
            output_components: components,
            output_timelines: self.time_outputs,
        })
    }
}
