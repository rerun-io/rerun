use arrow::datatypes::{Field as ArrowField, Fields as ArrowFields};

use itertools::Itertools as _;
use nohash_hasher::IntSet;
use re_log_types::EntityPath;

use crate::{
    ColumnDescriptor, ColumnDescriptorRef, ColumnKind, ComponentColumnDescriptor,
    IndexColumnDescriptor, RowIdColumnDescriptor, SorbetError,
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

    /// Returns all columns, including the `row_id` column.
    ///
    /// See also [`Self::indices_and_components`].
    pub fn descriptors(&self) -> impl Iterator<Item = ColumnDescriptorRef<'_>> + '_ {
        self.row_id
            .iter()
            .map(ColumnDescriptorRef::from)
            .chain(self.indices.iter().map(ColumnDescriptorRef::from))
            .chain(self.components.iter().map(ColumnDescriptorRef::from))
    }

    /// Returns all indices and then all components;
    /// skipping the `row_id` column.
    ///
    /// See also [`Self::get_index_or_component`].
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

    /// Index the index- and component columns, ignoring the `row_id` column completely.
    ///
    /// That is, `get_index_or_component(0)` will return the first index column (if any; otherwise
    /// the first component column).
    ///
    /// See also [`Self::indices_and_components`].
    pub fn get_index_or_component(&self, index_ignoring_row_id: usize) -> Option<ColumnDescriptor> {
        if index_ignoring_row_id < self.indices.len() {
            Some(ColumnDescriptor::Time(
                self.indices[index_ignoring_row_id].clone(),
            ))
        } else {
            self.components
                .get(index_ignoring_row_id - self.indices.len())
                .cloned()
                .map(ColumnDescriptor::Component)
        }
    }

    pub fn arrow_fields(&self, batch_type: crate::BatchType) -> Vec<ArrowField> {
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
                .map(|column| column.to_arrow_field(batch_type)),
        );
        fields
    }

    /// Keep only the component columns that satisfy the given predicate.
    #[must_use]
    #[inline]
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
                        let err = format!(
                            "RowId column must be the first column; but the columns were: {:?}",
                            fields.iter().map(|f| f.name()).collect_vec()
                        );
                        return Err(SorbetError::custom(err));
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
