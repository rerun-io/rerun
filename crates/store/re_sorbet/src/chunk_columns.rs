use arrow::datatypes::{Field as ArrowField, Fields as ArrowFields};
use re_log_types::EntityPath;

use crate::{
    BatchType, ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor,
    RowIdColumnDescriptor, SorbetColumnDescriptors, SorbetError,
};

/// Requires a specific ordering of the columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkColumnDescriptors {
    /// The primary row id column.
    pub row_id: RowIdColumnDescriptor,

    /// Index columns (timelines).
    pub indices: Vec<IndexColumnDescriptor>,

    /// The actual component data
    pub components: Vec<ComponentColumnDescriptor>,
}

impl ChunkColumnDescriptors {
    /// Debug-only sanity check.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        for component in &self.components {
            component.sanity_check();
        }
    }

    /// Returns all indices and then all components;
    /// skipping the `row_id` column.
    ///
    /// See also [`Self::get_index_or_component`].
    // TODO(#9922): stop ignoring row_id
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
    // TODO(#9922): stop ignoring row_id
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

    /// Keep only the component columns that satisfy the given predicate.
    #[must_use]
    #[inline]
    pub fn filter_components(mut self, keep: impl Fn(&ComponentColumnDescriptor) -> bool) -> Self {
        self.components.retain(keep);
        self
    }

    pub fn arrow_fields(&self) -> Vec<ArrowField> {
        std::iter::once(self.row_id.to_arrow_field())
            .chain(self.indices.iter().map(|c| c.to_arrow_field()))
            .chain(
                self.components
                    .iter()
                    .map(|c| c.to_arrow_field(BatchType::Dataframe)),
            )
            .collect()
    }
}

impl ChunkColumnDescriptors {
    pub fn try_from_arrow_fields(
        chunk_entity_path: Option<&EntityPath>,
        fields: &ArrowFields,
    ) -> Result<Self, SorbetError> {
        Self::try_from(SorbetColumnDescriptors::try_from_arrow_fields(
            chunk_entity_path,
            fields,
        )?)
    }
}

impl TryFrom<SorbetColumnDescriptors> for ChunkColumnDescriptors {
    type Error = SorbetError;

    fn try_from(columns: SorbetColumnDescriptors) -> Result<Self, Self::Error> {
        let SorbetColumnDescriptors { columns } = columns;

        let mut row_ids = Vec::new();
        let mut indices = Vec::new();
        let mut components = Vec::new();

        for column in &columns {
            match column.clone() {
                ColumnDescriptor::RowId(descr) => {
                    if indices.is_empty() && components.is_empty() {
                        row_ids.push(descr);
                    } else {
                        let err = format!(
                            "RowId column must be the first column; but the columns were: {columns:?}"
                        );
                        return Err(SorbetError::InvalidColumnOrder(err));
                    }
                }

                ColumnDescriptor::Time(descr) => {
                    if components.is_empty() {
                        indices.push(descr);
                    } else {
                        return Err(SorbetError::InvalidColumnOrder(
                            "Index columns must come before any data columns".to_owned(),
                        ));
                    }
                }

                ColumnDescriptor::Component(descr) => {
                    components.push(descr);
                }
            }
        }

        if row_ids.len() > 1 {
            return Err(SorbetError::MultipleRowIdColumns(row_ids.len()));
        }

        let row_id = row_ids.pop().ok_or_else(|| {
            re_log::debug!("Missing RowId column, but had these columns: {columns:#?}");
            SorbetError::MissingRowIdColumn
        })?;

        Ok(Self {
            row_id,
            indices,
            components,
        })
    }
}

impl From<ChunkColumnDescriptors> for SorbetColumnDescriptors {
    fn from(columns: ChunkColumnDescriptors) -> Self {
        let ChunkColumnDescriptors {
            row_id,
            indices,
            components,
        } = columns;

        let columns = itertools::chain!(
            std::iter::once(ColumnDescriptor::RowId(row_id.clone())),
            indices.iter().cloned().map(ColumnDescriptor::Time),
            components.iter().cloned().map(ColumnDescriptor::Component),
        )
        .collect();

        Self { columns }
    }
}
