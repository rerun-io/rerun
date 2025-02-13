use arrow::datatypes::{Field as ArrowField, Fields as ArrowFields};

use nohash_hasher::IntSet;
use re_log_types::EntityPath;

use crate::{
    ColumnDescriptor, ColumnKind, ComponentColumnDescriptor, IndexColumnDescriptor,
    RowIdColumnDescriptor, SorbetError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorbetColumnDescriptors {
    /// The primary row id column.
    /// If present, it is always the first column.
    pub row_id: Option<RowIdColumnDescriptor>,

    /// Index columns (timelines).
    pub indices: Vec<IndexColumnDescriptor>,

    /// The actual component data
    pub components: Vec<ComponentColumnDescriptor>,
}

impl SorbetColumnDescriptors {
    /// Debug-only sanity check.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        for component in &self.components {
            component.sanity_check();
        }
    }

    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        let Self {
            row_id,
            indices,
            components,
        } = self;
        row_id.is_some() as usize + indices.len() + components.len()
    }

    /// All unique entity paths present in the view contents.
    pub fn entity_paths(&self) -> IntSet<EntityPath> {
        self.components
            .iter()
            .map(|col| col.entity_path.clone())
            .collect()
    }

    /// Returns all indices and then all components;
    /// skipping the `row_id` column.
    pub fn indices_and_components(&self) -> Vec<ColumnDescriptor> {
        itertools::chain!(
            self.indices.iter().cloned().map(ColumnDescriptor::Time),
            self.components
                .iter()
                .cloned()
                .map(ColumnDescriptor::Component),
        )
        .collect()
    }

    pub fn arrow_fields(&self) -> Vec<ArrowField> {
        let Self {
            row_id,
            indices,
            components,
        } = self;
        let mut fields: Vec<ArrowField> = Vec::with_capacity(self.num_columns());
        if let Some(row_id) = row_id {
            fields.push(row_id.to_arrow_field());
        }
        fields.extend(indices.iter().map(|column| column.to_arrow_field()));
        fields.extend(
            components
                .iter()
                .map(|column| column.to_arrow_field(crate::BatchType::Chunk)),
        );
        fields
    }

    /// Keep only the component columns that satisfy the given predicate.
    #[must_use]
    pub fn filter_components(mut self, keep: impl Fn(&ComponentColumnDescriptor) -> bool) -> Self {
        self.components.retain(keep);
        self
    }
}

impl SorbetColumnDescriptors {
    pub fn try_from_arrow_fields(
        chunk_entity_path: Option<&EntityPath>,
        fields: &ArrowFields,
    ) -> Result<Self, SorbetError> {
        let mut row_ids = Vec::new();
        let mut indices = Vec::new();
        let mut components = Vec::new();

        for field in fields {
            let field = field.as_ref();
            let column_kind = ColumnKind::try_from(field)?;
            match column_kind {
                ColumnKind::RowId => {
                    if indices.is_empty() && components.is_empty() {
                        row_ids.push(RowIdColumnDescriptor::try_from(field)?);
                    } else {
                        return Err(SorbetError::custom("RowId column must be the first column"));
                    }
                }

                ColumnKind::Index => {
                    if components.is_empty() {
                        indices.push(IndexColumnDescriptor::try_from(field)?);
                    } else {
                        return Err(SorbetError::custom(
                            "Index columns must come before any data columns",
                        ));
                    }
                }

                ColumnKind::Component => {
                    components.push(ComponentColumnDescriptor::from_arrow_field(
                        chunk_entity_path,
                        field,
                    ));
                }
            }
        }

        if row_ids.len() > 1 {
            return Err(SorbetError::custom(
                "Multiple row_id columns are not supported",
            ));
        }

        Ok(Self {
            row_id: row_ids.pop(),
            indices,
            components,
        })
    }
}
