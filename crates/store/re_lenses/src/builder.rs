//! Builder API for constructing lenses.

use re_chunk::{ComponentIdentifier, EntityPath, TimelineName};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::ComponentDescriptor;

use crate::ast::{OneToMany, OneToOne, Static};
use crate::{LensError, Op, ast};

/// Builder for lenses with support for multiple output modes.
#[must_use]
pub struct LensBuilder {
    input: ast::InputColumn,
    outputs: Vec<ast::LensKind>,
}

impl LensBuilder {
    pub(crate) fn new(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
    ) -> Self {
        Self {
            input: ast::InputColumn {
                entity_path_filter,
                component: component.into(),
            },
            outputs: vec![],
        }
    }

    /// Adds a temporal output with 1:1 row mapping.
    ///
    /// Each input row produces exactly one output row. Outputs inherit time columns from
    /// the input, plus any additional time columns specified via `.time()`.
    ///
    /// The output will use the same entity path as the input.
    pub fn output_columns(
        mut self,
        builder: impl FnOnce(ColumnsBuilder) -> ColumnsBuilder,
    ) -> Result<Self, LensError> {
        let output_builder = ColumnsBuilder::new(ast::TargetEntity::SameAsInput);
        let output = builder(output_builder).build(&self.input)?;
        self.outputs.push(output);
        Ok(self)
    }

    /// Adds a temporal output with 1:1 row mapping at a specific entity path.
    ///
    /// Each input row produces exactly one output row. Outputs inherit time columns from
    /// the input, plus any additional time columns specified via `.time()`.
    pub fn output_columns_at(
        mut self,
        entity_path: impl Into<EntityPath>,
        builder: impl FnOnce(ColumnsBuilder) -> ColumnsBuilder,
    ) -> Result<Self, LensError> {
        let output_builder = ColumnsBuilder::new(ast::TargetEntity::Explicit(entity_path.into()));
        let output = builder(output_builder).build(&self.input)?;
        self.outputs.push(output);
        Ok(self)
    }

    /// Adds a static output (timeless data).
    ///
    /// Creates data that does not change over time and has no associated time columns.
    ///
    /// The output will use the same entity path as the input.
    pub fn output_static_columns(
        mut self,
        builder: impl FnOnce(StaticColumnsBuilder) -> StaticColumnsBuilder,
    ) -> Result<Self, LensError> {
        let output_builder = StaticColumnsBuilder::new(ast::TargetEntity::SameAsInput);
        let output = builder(output_builder).build(&self.input)?;
        self.outputs.push(output);
        Ok(self)
    }

    /// Adds a static output (timeless data) at a specific entity path.
    ///
    /// Creates data that does not change over time and has no associated time columns.
    pub fn output_static_columns_at(
        mut self,
        entity_path: impl Into<EntityPath>,
        builder: impl FnOnce(StaticColumnsBuilder) -> StaticColumnsBuilder,
    ) -> Result<Self, LensError> {
        let output_builder =
            StaticColumnsBuilder::new(ast::TargetEntity::Explicit(entity_path.into()));
        let output = builder(output_builder).build(&self.input)?;
        self.outputs.push(output);
        Ok(self)
    }

    /// Adds a temporal output with 1:N row mapping (scatter).
    ///
    /// Each input row produces multiple output rows at the same timepoint. The timepoint
    /// is replicated/scattered across the output rows. Useful for flattening lists or
    /// exploding batches.
    ///
    /// The output will use the same entity path as the input.
    pub fn output_scatter_columns(
        mut self,
        builder: impl FnOnce(ScatterColumnsBuilder) -> ScatterColumnsBuilder,
    ) -> Result<Self, LensError> {
        let output_builder = ScatterColumnsBuilder::new(ast::TargetEntity::SameAsInput);
        let output = builder(output_builder).build(&self.input)?;
        self.outputs.push(output);
        Ok(self)
    }

    /// Adds a temporal output with 1:N row mapping (scatter) at a specific entity path.
    ///
    /// Each input row produces multiple output rows at the same timepoint. The timepoint
    /// is replicated/scattered across the output rows. Useful for flattening lists or
    /// exploding batches.
    pub fn output_scatter_columns_at(
        mut self,
        entity_path: impl Into<EntityPath>,
        builder: impl FnOnce(ScatterColumnsBuilder) -> ScatterColumnsBuilder,
    ) -> Result<Self, LensError> {
        let output_builder =
            ScatterColumnsBuilder::new(ast::TargetEntity::Explicit(entity_path.into()));
        let output = builder(output_builder).build(&self.input)?;
        self.outputs.push(output);
        Ok(self)
    }

    /// Finalizes this builder and returns the corresponding lens.
    pub fn build(self) -> ast::Lens {
        ast::Lens {
            input: self.input,
            outputs: self.outputs,
        }
    }
}

// ==================== Output Builders ====================

/// Builder for temporal outputs with 1:1 row mapping.
///
/// Each input row produces exactly one output row. Outputs inherit time columns
/// from the input, plus any additional time columns specified.
#[must_use]
pub struct ColumnsBuilder {
    target_entity: ast::TargetEntity,
    components: Vec<ast::ComponentOutput>,
    time_outputs: Vec<ast::TimeOutput>,
}

impl ColumnsBuilder {
    fn new(target_entity: ast::TargetEntity) -> Self {
        Self {
            target_entity,
            components: vec![],
            time_outputs: vec![],
        }
    }

    /// Adds a component output column.
    ///
    /// # Arguments
    /// * `component_descr` - The descriptor for the output component
    /// * `ops` - Sequence of operations to apply to transform the input column
    pub fn component(
        mut self,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        self.components.push(ast::ComponentOutput {
            component_descr,
            ops: ops.into_iter().collect(),
        });
        self
    }

    /// Adds a time extraction.
    ///
    /// Extracts data from the input column to create a new time column for the output rows.
    ///
    /// # Arguments
    /// * `timeline_name` - Name of the timeline to create
    /// * `timeline_type` - Type of timeline (Sequence or Time)
    /// * `ops` - Sequence of operations to extract time values (must produce [`arrow::array::Int64Array`])
    pub fn time(
        mut self,
        timeline_name: impl Into<TimelineName>,
        timeline_type: TimeType,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        self.time_outputs.push(ast::TimeOutput {
            timeline_name: timeline_name.into(),
            timeline_type,
            ops: ops.into_iter().collect(),
        });
        self
    }

    /// Builds a [`ast::LensKind`], the `input` is passed for providing contextualized errors.
    fn build(self, input: &ast::InputColumn) -> Result<ast::LensKind, LensError> {
        Ok(ast::LensKind::Columns(OneToOne {
            target_entity: self.target_entity,
            components: self.components.try_into().map_err(|_err| {
                LensError::MissingOutputComponent {
                    input_filter: input.entity_path_filter.clone(),
                    input_component: input.component,
                }
            })?,
            times: self.time_outputs,
        }))
    }
}

/// Builder for static outputs (timeless data).
///
/// Creates data that does not change over time. Static outputs have no associated time columns.
#[must_use]
pub struct StaticColumnsBuilder {
    target_entity: ast::TargetEntity,
    components: Vec<ast::ComponentOutput>,
}

impl StaticColumnsBuilder {
    fn new(target_entity: ast::TargetEntity) -> Self {
        Self {
            target_entity,
            components: vec![],
        }
    }

    /// Adds a component output column.
    ///
    /// # Arguments
    /// * `component_descr` - The descriptor for the output component
    /// * `ops` - Sequence of operations to apply to transform the input column
    pub fn component(
        mut self,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        self.components.push(ast::ComponentOutput {
            component_descr,
            ops: ops.into_iter().collect(),
        });
        self
    }

    /// Builds a [`ast::LensKind`], the `input` is passed for providing contextualized errors.
    fn build(self, input: &ast::InputColumn) -> Result<ast::LensKind, LensError> {
        Ok(ast::LensKind::StaticColumns(Static {
            target_entity: self.target_entity,
            components: self.components.try_into().map_err(|_err| {
                LensError::MissingOutputComponent {
                    input_filter: input.entity_path_filter.clone(),
                    input_component: input.component,
                }
            })?,
        }))
    }
}

/// Builder for temporal outputs with 1:N row mapping (scatter).
///
/// Each input row produces multiple output rows at the same timepoint. The timepoint
/// is replicated/scattered across all output rows. This is useful for flattening lists
/// or exploding batches while maintaining temporal alignment.
#[must_use]
pub struct ScatterColumnsBuilder {
    target_entity: ast::TargetEntity,
    components: Vec<ast::ComponentOutput>,
    time_outputs: Vec<ast::TimeOutput>,
}

impl ScatterColumnsBuilder {
    fn new(target_entity: ast::TargetEntity) -> Self {
        Self {
            target_entity,
            components: vec![],
            time_outputs: vec![],
        }
    }

    /// Adds a component output column.
    ///
    /// # Arguments
    /// * `component_descr` - The descriptor for the output component
    /// * `ops` - Sequence of operations to apply to transform the input column
    pub fn component(
        mut self,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        self.components.push(ast::ComponentOutput {
            component_descr,
            ops: ops.into_iter().collect(),
        });
        self
    }

    /// Adds a time extraction.
    ///
    /// Extracts data from the input column to create a new time column for the output rows.
    ///
    /// # Arguments
    /// * `timeline_name` - Name of the timeline to create
    /// * `timeline_type` - Type of timeline (Sequence or Time)
    /// * `ops` - Sequence of operations to extract time values (must produce [`arrow::array::Int64Array`])
    pub fn time(
        mut self,
        timeline_name: impl Into<TimelineName>,
        timeline_type: TimeType,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        self.time_outputs.push(ast::TimeOutput {
            timeline_name: timeline_name.into(),
            timeline_type,
            ops: ops.into_iter().collect(),
        });
        self
    }

    /// Builds a [`ast::LensKind`], the `input` is passed for providing contextualized errors.
    fn build(self, input: &ast::InputColumn) -> Result<ast::LensKind, LensError> {
        Ok(ast::LensKind::ScatterColumns(OneToMany {
            target_entity: self.target_entity,
            components: self.components.try_into().map_err(|_err| {
                LensError::MissingOutputComponent {
                    input_filter: input.entity_path_filter.clone(),
                    input_component: input.component,
                }
            })?,
            times: self.time_outputs,
        }))
    }
}
